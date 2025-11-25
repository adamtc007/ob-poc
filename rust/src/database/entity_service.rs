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

/// Fields for creating a limited company
#[derive(Debug, Clone)]
pub struct NewLimitedCompanyFields {
    pub name: String,
    pub jurisdiction: Option<String>,
    pub registration_number: Option<String>,
    pub incorporation_date: Option<NaiveDate>,
    pub registered_address: Option<String>,
    pub business_nature: Option<String>,
}

/// Fields for creating a partnership
#[derive(Debug, Clone)]
pub struct NewPartnershipFields {
    pub name: String,
    pub jurisdiction: Option<String>,
    pub partnership_type: Option<String>, // LP, LLP, GP
    pub formation_date: Option<NaiveDate>,
    pub principal_place_business: Option<String>,
    pub partnership_agreement_date: Option<NaiveDate>,
}

/// Fields for creating a trust
#[derive(Debug, Clone)]
pub struct NewTrustFields {
    pub name: String,
    pub jurisdiction: String,
    pub trust_type: Option<String>, // Discretionary, Fixed, Unit
    pub establishment_date: Option<NaiveDate>,
    pub trust_deed_date: Option<NaiveDate>,
    pub trust_purpose: Option<String>,
    pub governing_law: Option<String>,
}

/// Limited company row - matches DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LimitedCompanyRow {
    pub limited_company_id: Uuid,
    pub company_name: String,
    pub registration_number: Option<String>,
    pub jurisdiction: Option<String>,
    pub incorporation_date: Option<NaiveDate>,
    pub registered_address: Option<String>,
    pub business_nature: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Partnership row - matches DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PartnershipRow {
    pub partnership_id: Uuid,
    pub partnership_name: String,
    pub partnership_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub formation_date: Option<NaiveDate>,
    pub principal_place_business: Option<String>,
    pub partnership_agreement_date: Option<NaiveDate>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Trust row - matches DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrustRow {
    pub trust_id: Uuid,
    pub trust_name: String,
    pub trust_type: Option<String>,
    pub jurisdiction: String,
    pub establishment_date: Option<NaiveDate>,
    pub trust_deed_date: Option<NaiveDate>,
    pub trust_purpose: Option<String>,
    pub governing_law: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// CBU entity role row - matches DB schema
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuEntityRoleRow {
    pub cbu_entity_role_id: Uuid,
    pub cbu_id: Uuid,
    pub entity_id: Uuid,
    pub role_id: Uuid,
    pub created_at: Option<DateTime<Utc>>,
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

    // =========================================================================
    // Limited Company Operations
    // =========================================================================

    /// Create a limited company
    /// Returns limited_company_id
    pub async fn create_limited_company(&self, fields: &NewLimitedCompanyFields) -> Result<Uuid> {
        let limited_company_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_limited_companies (
                limited_company_id, company_name, registration_number,
                jurisdiction, incorporation_date, registered_address,
                business_nature, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            "#,
        )
        .bind(limited_company_id)
        .bind(&fields.name)
        .bind(&fields.registration_number)
        .bind(&fields.jurisdiction)
        .bind(fields.incorporation_date)
        .bind(&fields.registered_address)
        .bind(&fields.business_nature)
        .execute(&self.pool)
        .await
        .context("Failed to create limited company")?;

        info!(
            "Created limited company: {} (id: {})",
            fields.name, limited_company_id
        );

        Ok(limited_company_id)
    }

    /// Get limited company by ID
    pub async fn get_limited_company(
        &self,
        limited_company_id: Uuid,
    ) -> Result<Option<LimitedCompanyRow>> {
        sqlx::query_as::<_, LimitedCompanyRow>(
            r#"
            SELECT limited_company_id, company_name, registration_number,
                   jurisdiction, incorporation_date, registered_address,
                   business_nature, created_at, updated_at
            FROM "ob-poc".entity_limited_companies
            WHERE limited_company_id = $1
            "#,
        )
        .bind(limited_company_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get limited company")
    }

    /// List limited companies
    pub async fn list_limited_companies(
        &self,
        jurisdiction: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<LimitedCompanyRow>> {
        let results = if let Some(j) = jurisdiction {
            sqlx::query_as::<_, LimitedCompanyRow>(
                r#"
                SELECT limited_company_id, company_name, registration_number,
                       jurisdiction, incorporation_date, registered_address,
                       business_nature, created_at, updated_at
                FROM "ob-poc".entity_limited_companies
                WHERE jurisdiction = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(j)
            .bind(limit.unwrap_or(100))
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, LimitedCompanyRow>(
                r#"
                SELECT limited_company_id, company_name, registration_number,
                       jurisdiction, incorporation_date, registered_address,
                       business_nature, created_at, updated_at
                FROM "ob-poc".entity_limited_companies
                ORDER BY created_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit.unwrap_or(100))
            .fetch_all(&self.pool)
            .await
        }
        .context("Failed to list limited companies")?;

        Ok(results)
    }

    // =========================================================================
    // Partnership Operations
    // =========================================================================

    /// Create a partnership
    /// Returns partnership_id
    pub async fn create_partnership(&self, fields: &NewPartnershipFields) -> Result<Uuid> {
        let partnership_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_partnerships (
                partnership_id, partnership_name, partnership_type,
                jurisdiction, formation_date, principal_place_business,
                partnership_agreement_date, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            "#,
        )
        .bind(partnership_id)
        .bind(&fields.name)
        .bind(&fields.partnership_type)
        .bind(&fields.jurisdiction)
        .bind(fields.formation_date)
        .bind(&fields.principal_place_business)
        .bind(fields.partnership_agreement_date)
        .execute(&self.pool)
        .await
        .context("Failed to create partnership")?;

        info!(
            "Created partnership: {} (id: {})",
            fields.name, partnership_id
        );

        Ok(partnership_id)
    }

    /// Get partnership by ID
    pub async fn get_partnership(&self, partnership_id: Uuid) -> Result<Option<PartnershipRow>> {
        sqlx::query_as::<_, PartnershipRow>(
            r#"
            SELECT partnership_id, partnership_name, partnership_type,
                   jurisdiction, formation_date, principal_place_business,
                   partnership_agreement_date, created_at, updated_at
            FROM "ob-poc".entity_partnerships
            WHERE partnership_id = $1
            "#,
        )
        .bind(partnership_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get partnership")
    }

    /// List partnerships
    pub async fn list_partnerships(
        &self,
        jurisdiction: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<PartnershipRow>> {
        let results = if let Some(j) = jurisdiction {
            sqlx::query_as::<_, PartnershipRow>(
                r#"
                SELECT partnership_id, partnership_name, partnership_type,
                       jurisdiction, formation_date, principal_place_business,
                       partnership_agreement_date, created_at, updated_at
                FROM "ob-poc".entity_partnerships
                WHERE jurisdiction = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(j)
            .bind(limit.unwrap_or(100))
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, PartnershipRow>(
                r#"
                SELECT partnership_id, partnership_name, partnership_type,
                       jurisdiction, formation_date, principal_place_business,
                       partnership_agreement_date, created_at, updated_at
                FROM "ob-poc".entity_partnerships
                ORDER BY created_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit.unwrap_or(100))
            .fetch_all(&self.pool)
            .await
        }
        .context("Failed to list partnerships")?;

        Ok(results)
    }

    // =========================================================================
    // Trust Operations
    // =========================================================================

    /// Create a trust
    /// Returns trust_id
    pub async fn create_trust(&self, fields: &NewTrustFields) -> Result<Uuid> {
        let trust_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_trusts (
                trust_id, trust_name, trust_type, jurisdiction,
                establishment_date, trust_deed_date, trust_purpose,
                governing_law, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            "#,
        )
        .bind(trust_id)
        .bind(&fields.name)
        .bind(&fields.trust_type)
        .bind(&fields.jurisdiction)
        .bind(fields.establishment_date)
        .bind(fields.trust_deed_date)
        .bind(&fields.trust_purpose)
        .bind(&fields.governing_law)
        .execute(&self.pool)
        .await
        .context("Failed to create trust")?;

        info!("Created trust: {} (id: {})", fields.name, trust_id);

        Ok(trust_id)
    }

    /// Get trust by ID
    pub async fn get_trust(&self, trust_id: Uuid) -> Result<Option<TrustRow>> {
        sqlx::query_as::<_, TrustRow>(
            r#"
            SELECT trust_id, trust_name, trust_type, jurisdiction,
                   establishment_date, trust_deed_date, trust_purpose,
                   governing_law, created_at, updated_at
            FROM "ob-poc".entity_trusts
            WHERE trust_id = $1
            "#,
        )
        .bind(trust_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get trust")
    }

    /// List trusts
    pub async fn list_trusts(
        &self,
        jurisdiction: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<TrustRow>> {
        let results = if let Some(j) = jurisdiction {
            sqlx::query_as::<_, TrustRow>(
                r#"
                SELECT trust_id, trust_name, trust_type, jurisdiction,
                       establishment_date, trust_deed_date, trust_purpose,
                       governing_law, created_at, updated_at
                FROM "ob-poc".entity_trusts
                WHERE jurisdiction = $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
            )
            .bind(j)
            .bind(limit.unwrap_or(100))
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, TrustRow>(
                r#"
                SELECT trust_id, trust_name, trust_type, jurisdiction,
                       establishment_date, trust_deed_date, trust_purpose,
                       governing_law, created_at, updated_at
                FROM "ob-poc".entity_trusts
                ORDER BY created_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit.unwrap_or(100))
            .fetch_all(&self.pool)
            .await
        }
        .context("Failed to list trusts")?;

        Ok(results)
    }

    // =========================================================================
    // CBU Entity Role Operations (Hub-Spoke Attachment)
    // =========================================================================

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

    /// Attach an entity to a CBU with a role
    /// Returns cbu_entity_role_id
    pub async fn attach_entity_to_cbu(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_name: &str,
    ) -> Result<Uuid> {
        let role_id = self.resolve_role_id(role_name).await?;
        let cbu_entity_role_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_entity_roles (
                cbu_entity_role_id, cbu_id, entity_id, role_id, created_at
            )
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (cbu_id, entity_id, role_id) DO NOTHING
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
            "Attached entity {} to CBU {} with role '{}'",
            entity_id, cbu_id, role_name
        );

        Ok(cbu_entity_role_id)
    }

    /// Detach an entity from a CBU (optionally for specific role)
    pub async fn detach_entity_from_cbu(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        role_name: Option<&str>,
    ) -> Result<u64> {
        let rows_affected = if let Some(role) = role_name {
            let role_id = self.resolve_role_id(role).await?;
            sqlx::query(
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
        .context("Failed to detach entity from CBU")?
        .rows_affected();

        info!(
            "Detached entity {} from CBU {} (rows: {})",
            entity_id, cbu_id, rows_affected
        );

        Ok(rows_affected)
    }

    /// List entities attached to a CBU (optionally filtered by role)
    pub async fn list_cbu_entities(
        &self,
        cbu_id: Uuid,
        role_name: Option<&str>,
    ) -> Result<Vec<CbuEntityRoleRow>> {
        let results = if let Some(role) = role_name {
            let role_id = self.resolve_role_id(role).await?;
            sqlx::query_as::<_, CbuEntityRoleRow>(
                r#"
                SELECT cbu_entity_role_id, cbu_id, entity_id, role_id, created_at
                FROM "ob-poc".cbu_entity_roles
                WHERE cbu_id = $1 AND role_id = $2
                ORDER BY created_at DESC
                "#,
            )
            .bind(cbu_id)
            .bind(role_id)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, CbuEntityRoleRow>(
                r#"
                SELECT cbu_entity_role_id, cbu_id, entity_id, role_id, created_at
                FROM "ob-poc".cbu_entity_roles
                WHERE cbu_id = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(cbu_id)
            .fetch_all(&self.pool)
            .await
        }
        .context("Failed to list CBU entities")?;

        Ok(results)
    }

    /// Update entity role within a CBU
    pub async fn update_cbu_entity_role(
        &self,
        cbu_id: Uuid,
        entity_id: Uuid,
        old_role_name: &str,
        new_role_name: &str,
    ) -> Result<bool> {
        let old_role_id = self.resolve_role_id(old_role_name).await?;
        let new_role_id = self.resolve_role_id(new_role_name).await?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".cbu_entity_roles
            SET role_id = $1
            WHERE cbu_id = $2 AND entity_id = $3 AND role_id = $4
            "#,
        )
        .bind(new_role_id)
        .bind(cbu_id)
        .bind(entity_id)
        .bind(old_role_id)
        .execute(&self.pool)
        .await
        .context("Failed to update CBU entity role")?;

        if result.rows_affected() > 0 {
            info!(
                "Updated entity {} role in CBU {} from '{}' to '{}'",
                entity_id, cbu_id, old_role_name, new_role_name
            );
        }

        Ok(result.rows_affected() > 0)
    }
}

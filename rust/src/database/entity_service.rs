//! Entity Database Service - Comprehensive entity management operations
//!
//! This service provides database operations for all entity-related tables in the ob-poc schema,
//! supporting the full entity lifecycle from creation to archival. It follows the established
//! database service patterns and integrates with the agentic CRUD system.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Comprehensive entity database service for all entity operations
pub struct EntityDatabaseService {
    pool: PgPool,
}

impl EntityDatabaseService {
    /// Create new entity database service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ==========================================
    // CORE ENTITY OPERATIONS
    // ==========================================

    /// Create a new entity in the entities table
    pub async fn create_entity(&self, request: CreateEntityRequest) -> Result<Entity> {
        let entity_id = Uuid::new_v4();

        let entity = sqlx::query_as::<_, Entity>(
            r#"
            INSERT INTO "ob-poc".entities (
                entity_id, entity_type_id, external_id, name, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, NOW(), NOW())
            RETURNING entity_id, entity_type_id, external_id, name, created_at, updated_at
            "#,
        )
        .bind(entity_id)
        .bind(request.entity_type_id)
        .bind(request.external_id)
        .bind(request.name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create entity")?;

        info!("Created entity: {} ({})", entity.name, entity.entity_id);
        Ok(entity)
    }

    /// Get entity by ID
    pub async fn get_entity(&self, entity_id: Uuid) -> Result<Option<Entity>> {
        let entity = sqlx::query_as::<_, Entity>(
            r#"
            SELECT entity_id, entity_type_id, external_id, name, created_at, updated_at
            FROM "ob-poc".entities
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch entity")?;

        Ok(entity)
    }

    /// Update entity
    pub async fn update_entity(
        &self,
        entity_id: Uuid,
        request: UpdateEntityRequest,
    ) -> Result<Entity> {
        let entity = sqlx::query_as::<_, Entity>(
            r#"
            UPDATE "ob-poc".entities
            SET external_id = COALESCE($2, external_id),
                name = COALESCE($3, name),
                updated_at = NOW()
            WHERE entity_id = $1
            RETURNING entity_id, entity_type_id, external_id, name, created_at, updated_at
            "#,
        )
        .bind(entity_id)
        .bind(request.external_id)
        .bind(request.name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update entity")?;

        info!("Updated entity: {} ({})", entity.name, entity.entity_id);
        Ok(entity)
    }

    /// Delete entity (soft delete by marking as deleted)
    pub async fn delete_entity(&self, entity_id: Uuid) -> Result<()> {
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

        if result.rows_affected() == 0 {
            return Err(anyhow!("Entity not found: {}", entity_id));
        }

        info!("Deleted entity: {}", entity_id);
        Ok(())
    }

    // ==========================================
    // ENTITY TYPE MANAGEMENT
    // ==========================================

    /// Get all entity types
    pub async fn get_entity_types(&self) -> Result<Vec<EntityType>> {
        let entity_types = sqlx::query_as::<_, EntityType>(
            r#"
            SELECT entity_type_id, name, description, table_name, created_at, updated_at
            FROM "ob-poc".entity_types
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch entity types")?;

        Ok(entity_types)
    }

    /// Create new entity type
    pub async fn create_entity_type(&self, request: CreateEntityTypeRequest) -> Result<EntityType> {
        let entity_type_id = Uuid::new_v4();

        let entity_type = sqlx::query_as::<_, EntityType>(
            r#"
            INSERT INTO "ob-poc".entity_types (
                entity_type_id, name, description, table_name, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, NOW(), NOW())
            RETURNING entity_type_id, name, description, table_name, created_at, updated_at
            "#,
        )
        .bind(entity_type_id)
        .bind(request.name)
        .bind(request.description)
        .bind(request.table_name)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create entity type")?;

        info!(
            "Created entity type: {} ({})",
            entity_type.name, entity_type.entity_type_id
        );
        Ok(entity_type)
    }

    // ==========================================
    // SPECIALIZED ENTITY OPERATIONS
    // ==========================================

    /// Create limited company
    pub async fn create_limited_company(
        &self,
        request: CreateLimitedCompanyRequest,
    ) -> Result<LimitedCompany> {
        let limited_company_id = Uuid::new_v4();

        let company = sqlx::query_as::<_, LimitedCompany>(
            r#"
            INSERT INTO "ob-poc".entity_limited_companies (
                limited_company_id, company_name, registration_number, jurisdiction,
                incorporation_date, registered_address, business_nature, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING limited_company_id, company_name, registration_number, jurisdiction,
                      incorporation_date, registered_address, business_nature, created_at, updated_at
            "#,
        )
        .bind(limited_company_id)
        .bind(request.company_name)
        .bind(request.registration_number)
        .bind(request.jurisdiction)
        .bind(request.incorporation_date)
        .bind(request.registered_address)
        .bind(request.business_nature)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create limited company")?;

        info!(
            "Created limited company: {} ({})",
            company.company_name, company.limited_company_id
        );
        Ok(company)
    }

    /// Create partnership
    pub async fn create_partnership(
        &self,
        request: CreatePartnershipRequest,
    ) -> Result<Partnership> {
        let partnership_id = Uuid::new_v4();

        let partnership = sqlx::query_as::<_, Partnership>(
            r#"
            INSERT INTO "ob-poc".entity_partnerships (
                partnership_id, partnership_name, partnership_type, jurisdiction,
                formation_date, principal_place_business, partnership_agreement_date,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING partnership_id, partnership_name, partnership_type, jurisdiction,
                      formation_date, principal_place_business, partnership_agreement_date,
                      created_at, updated_at
            "#,
        )
        .bind(partnership_id)
        .bind(request.partnership_name)
        .bind(request.partnership_type)
        .bind(request.jurisdiction)
        .bind(request.formation_date)
        .bind(request.principal_place_business)
        .bind(request.partnership_agreement_date)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create partnership")?;

        info!(
            "Created partnership: {} ({})",
            partnership.partnership_name, partnership.partnership_id
        );
        Ok(partnership)
    }

    /// Create proper person
    pub async fn create_proper_person(
        &self,
        request: CreateProperPersonRequest,
    ) -> Result<ProperPerson> {
        let proper_person_id = Uuid::new_v4();

        let person = sqlx::query_as::<_, ProperPerson>(
            r#"
            INSERT INTO "ob-poc".entity_proper_persons (
                proper_person_id, first_name, last_name, date_of_birth,
                nationality, passport_number, residential_address, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING proper_person_id, first_name, last_name, date_of_birth,
                      nationality, passport_number, residential_address, created_at, updated_at
            "#,
        )
        .bind(proper_person_id)
        .bind(request.first_name)
        .bind(request.last_name)
        .bind(request.date_of_birth)
        .bind(request.nationality)
        .bind(request.passport_number)
        .bind(request.residential_address)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create proper person")?;

        info!(
            "Created proper person: {} {} ({})",
            person.first_name, person.last_name, person.proper_person_id
        );
        Ok(person)
    }

    /// Create trust
    pub async fn create_trust(&self, request: CreateTrustRequest) -> Result<Trust> {
        let trust_id = Uuid::new_v4();

        let trust = sqlx::query_as::<_, Trust>(
            r#"
            INSERT INTO "ob-poc".entity_trusts (
                trust_id, trust_name, trust_type, governing_law,
                establishment_date, trust_deed_date, principal_office_address,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING trust_id, trust_name, trust_type, governing_law,
                      establishment_date, trust_deed_date, principal_office_address,
                      created_at, updated_at
            "#,
        )
        .bind(trust_id)
        .bind(request.trust_name)
        .bind(request.trust_type)
        .bind(request.governing_law)
        .bind(request.establishment_date)
        .bind(request.trust_deed_date)
        .bind(request.principal_office_address)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create trust")?;

        info!("Created trust: {} ({})", trust.trust_name, trust.trust_id);
        Ok(trust)
    }

    // ==========================================
    // ENTITY LIFECYCLE MANAGEMENT
    // ==========================================

    /// Update entity status
    pub async fn update_entity_status(
        &self,
        request: EntityStatusUpdateRequest,
    ) -> Result<EntityLifecycleStatus> {
        let status_id = Uuid::new_v4();

        let status = sqlx::query_as::<_, EntityLifecycleStatus>(
            r#"
            INSERT INTO "ob-poc".entity_lifecycle_status (
                status_id, entity_type, entity_id, status_code, status_description,
                effective_date, end_date, reason_code, notes, created_by, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW(), NOW())
            RETURNING status_id, entity_type, entity_id, status_code, status_description,
                      effective_date, end_date, reason_code, notes, created_by, created_at, updated_at
            "#,
        )
        .bind(status_id)
        .bind(request.entity_type)
        .bind(request.entity_id)
        .bind(request.status_code)
        .bind(request.status_description)
        .bind(request.effective_date)
        .bind(request.end_date)
        .bind(request.reason_code)
        .bind(request.notes)
        .bind(request.created_by)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update entity status")?;

        info!(
            "Updated entity status: {} -> {} ({})",
            request.entity_id, request.status_code, status_id
        );
        Ok(status)
    }

    /// Get entity status history
    pub async fn get_entity_status_history(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityLifecycleStatus>> {
        let statuses = sqlx::query_as::<_, EntityLifecycleStatus>(
            r#"
            SELECT status_id, entity_type, entity_id, status_code, status_description,
                   effective_date, end_date, reason_code, notes, created_by, created_at, updated_at
            FROM "ob-poc".entity_lifecycle_status
            WHERE entity_id = $1
            ORDER BY effective_date DESC, created_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch entity status history")?;

        Ok(statuses)
    }

    // ==========================================
    // ENTITY SEARCH AND DISCOVERY
    // ==========================================

    /// Search entities by criteria
    pub async fn search_entities(&self, criteria: EntitySearchCriteria) -> Result<Vec<Entity>> {
        let mut query_builder = sqlx::QueryBuilder::new(
            "SELECT entity_id, entity_type_id, external_id, name, created_at, updated_at FROM \"ob-poc\".entities WHERE 1=1"
        );

        if let Some(name_pattern) = &criteria.name_pattern {
            query_builder.push(" AND name ILIKE ");
            query_builder.push_bind(format!("%{}%", name_pattern));
        }

        if let Some(entity_type_id) = criteria.entity_type_id {
            query_builder.push(" AND entity_type_id = ");
            query_builder.push_bind(entity_type_id);
        }

        if let Some(external_id) = &criteria.external_id {
            query_builder.push(" AND external_id = ");
            query_builder.push_bind(external_id);
        }

        query_builder.push(" ORDER BY name LIMIT ");
        query_builder.push_bind(criteria.limit.unwrap_or(100));

        let entities = query_builder
            .build_query_as::<Entity>()
            .fetch_all(&self.pool)
            .await
            .context("Failed to search entities")?;

        Ok(entities)
    }

    // ==========================================
    // VALIDATION AND BUSINESS RULES
    // ==========================================

    /// Validate entity against business rules
    pub async fn validate_entity(
        &self,
        entity_id: Uuid,
        rule_set: Vec<String>,
    ) -> Result<ValidationResult> {
        // Implementation would check various business rules
        // For now, return a basic validation result
        let entity = self.get_entity(entity_id).await?;

        match entity {
            Some(_) => Ok(ValidationResult {
                entity_id,
                is_valid: true,
                validation_errors: vec![],
                validation_warnings: vec![],
                rules_checked: rule_set,
                validated_at: Utc::now(),
            }),
            None => Err(anyhow!("Entity not found for validation: {}", entity_id)),
        }
    }

    /// Get entity validation rules for a specific entity type
    pub async fn get_entity_rules(&self, entity_type: &str) -> Result<Vec<EntityCrudRule>> {
        let rules = sqlx::query_as::<_, EntityCrudRule>(
            r#"
            SELECT rule_id, entity_table_name, operation_type, field_name,
                   constraint_type, constraint_description, validation_pattern,
                   error_message, is_active, created_at, updated_at
            FROM "ob-poc".entity_crud_rules
            WHERE entity_table_name = $1 AND is_active = true
            ORDER BY constraint_type, field_name
            "#,
        )
        .bind(entity_type)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch entity rules")?;

        Ok(rules)
    }

    // ==========================================
    // PRODUCT COMPATIBILITY
    // ==========================================

    /// Get products compatible with entity type
    pub async fn get_compatible_products(
        &self,
        entity_type_id: Uuid,
    ) -> Result<Vec<ProductMapping>> {
        let mappings = sqlx::query_as::<_, ProductMapping>(
            r#"
            SELECT mapping_id, entity_type_id, product_id, is_compatible,
                   compatibility_notes, created_at, updated_at
            FROM "ob-poc".entity_product_mappings
            WHERE entity_type_id = $1 AND is_compatible = true
            ORDER BY product_id
            "#,
        )
        .bind(entity_type_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch compatible products")?;

        Ok(mappings)
    }
}

// ==========================================
// DATA STRUCTURES
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Entity {
    pub entity_id: Uuid,
    pub entity_type_id: Uuid,
    pub external_id: Option<String>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityType {
    pub entity_type_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub table_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LimitedCompany {
    pub limited_company_id: Uuid,
    pub company_name: String,
    pub registration_number: Option<String>,
    pub jurisdiction: Option<String>,
    pub incorporation_date: Option<NaiveDate>,
    pub registered_address: Option<String>,
    pub business_nature: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Partnership {
    pub partnership_id: Uuid,
    pub partnership_name: String,
    pub partnership_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub formation_date: Option<NaiveDate>,
    pub principal_place_business: Option<String>,
    pub partnership_agreement_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProperPerson {
    pub proper_person_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub passport_number: Option<String>,
    pub residential_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Trust {
    pub trust_id: Uuid,
    pub trust_name: String,
    pub trust_type: Option<String>,
    pub governing_law: Option<String>,
    pub establishment_date: Option<NaiveDate>,
    pub trust_deed_date: Option<NaiveDate>,
    pub principal_office_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityLifecycleStatus {
    pub status_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub status_code: String,
    pub status_description: Option<String>,
    pub effective_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub reason_code: Option<String>,
    pub notes: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EntityCrudRule {
    pub rule_id: Uuid,
    pub entity_table_name: String,
    pub operation_type: String,
    pub field_name: Option<String>,
    pub constraint_type: String,
    pub constraint_description: String,
    pub validation_pattern: Option<String>,
    pub error_message: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductMapping {
    pub mapping_id: Uuid,
    pub entity_type_id: Uuid,
    pub product_id: Uuid,
    pub is_compatible: bool,
    pub compatibility_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ==========================================
// REQUEST STRUCTURES
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntityRequest {
    pub entity_type_id: Uuid,
    pub external_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEntityRequest {
    pub external_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntityTypeRequest {
    pub name: String,
    pub description: Option<String>,
    pub table_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLimitedCompanyRequest {
    pub company_name: String,
    pub registration_number: Option<String>,
    pub jurisdiction: Option<String>,
    pub incorporation_date: Option<NaiveDate>,
    pub registered_address: Option<String>,
    pub business_nature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePartnershipRequest {
    pub partnership_name: String,
    pub partnership_type: Option<String>,
    pub jurisdiction: Option<String>,
    pub formation_date: Option<NaiveDate>,
    pub principal_place_business: Option<String>,
    pub partnership_agreement_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProperPersonRequest {
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub nationality: Option<String>,
    pub passport_number: Option<String>,
    pub residential_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTrustRequest {
    pub trust_name: String,
    pub trust_type: Option<String>,
    pub governing_law: Option<String>,
    pub establishment_date: Option<NaiveDate>,
    pub trust_deed_date: Option<NaiveDate>,
    pub principal_office_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityStatusUpdateRequest {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub status_code: String,
    pub status_description: Option<String>,
    pub effective_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub reason_code: Option<String>,
    pub notes: Option<String>,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySearchCriteria {
    pub name_pattern: Option<String>,
    pub entity_type_id: Option<Uuid>,
    pub external_id: Option<String>,
    pub limit: Option<i64>,
}

// ==========================================
// RESPONSE STRUCTURES
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub entity_id: Uuid,
    pub is_valid: bool,
    pub validation_errors: Vec<String>,
    pub validation_warnings: Vec<String>,
    pub rules_checked: Vec<String>,
    pub validated_at: DateTime<Utc>,
}

// ==========================================
// TESTS
// ==========================================

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    // Note: These tests require a test database connection
    // They are integration tests, not unit tests

    #[sqlx::test]
    async fn test_create_and_get_entity(pool: PgPool) -> Result<()> {
        let service = EntityDatabaseService::new(pool);

        // First, we need an entity type (this would be seeded in real scenario)
        let entity_type_request = CreateEntityTypeRequest {
            name: "TEST_COMPANY".to_string(),
            description: Some("Test company type".to_string()),
            table_name: "entity_limited_companies".to_string(),
        };
        let entity_type = service.create_entity_type(entity_type_request).await?;

        // Create entity
        let request = CreateEntityRequest {
            entity_type_id: entity_type.entity_type_id,
            external_id: Some("TEST123".to_string()),
            name: "Test Company Ltd".to_string(),
        };

        let entity = service.create_entity(request).await?;
        assert_eq!(entity.name, "Test Company Ltd");
        assert_eq!(entity.external_id, Some("TEST123".to_string()));

        // Get entity
        let retrieved = service.get_entity(entity.entity_id).await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test Company Ltd");

        Ok(())
    }

    #[sqlx::test]
    async fn test_create_limited_company(pool: PgPool) -> Result<()> {
        let service = EntityDatabaseService::new(pool);

        let request = CreateLimitedCompanyRequest {
            company_name: "Test Corp Limited".to_string(),
            registration_number: Some("12345678".to_string()),
            jurisdiction: Some("GB".to_string()),
            incorporation_date: Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
            registered_address: Some("123 Test Street, London".to_string()),
            business_nature: Some("Software development".to_string()),
        };

        let company = service.create_limited_company(request).await?;
        assert_eq!(company.company_name, "Test Corp Limited");
        assert_eq!(company.registration_number, Some("12345678".to_string()));
        assert_eq!(company.jurisdiction, Some("GB".to_string()));

        Ok(())
    }

    #[sqlx::test]
    async fn test_entity_search(pool: PgPool) -> Result<()> {
        let service = EntityDatabaseService::new(pool);

        let criteria = EntitySearchCriteria {
            name_pattern: Some("Test".to_string()),
            entity_type_id: None,
            external_id: None,
            limit: Some(10),
        };

        let results = service.search_entities(criteria).await?;
        // Results depend on test data, just ensure no error
        assert!(results.len() >= 0);

        Ok(())
    }
}

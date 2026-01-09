//! BODS Service - Operations for BODS-aligned tables
//!
//! Handles entity_identifiers (LEI spine), gleif_relationships (corporate hierarchy),
//! bods_interest_types (reference data), and person_pep_status.
//!
//! Key design principle: GLEIF hierarchy (accounting consolidation) is SEPARATE
//! from beneficial ownership (entity_relationships for KYC/AML).

use anyhow::{Context, Result};
use chrono::NaiveDate;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

use super::bods_types::*;

/// Service for BODS-related operations
#[derive(Clone, Debug)]
pub struct BodsService {
    pool: PgPool,
}

impl BodsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // =========================================================================
    // Entity Identifiers (LEI Spine)
    // =========================================================================

    /// Attach an identifier to an entity (LEI, BIC, ISIN, REG_NUM, etc.)
    pub async fn attach_identifier(&self, fields: &NewEntityIdentifier) -> Result<Uuid> {
        let identifier_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_identifiers
                (identifier_id, entity_id, identifier_type, identifier_value,
                 issuing_authority, is_primary, source, scheme_name, uri,
                 created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW(), NOW())
            ON CONFLICT (entity_id, identifier_type, identifier_value) DO UPDATE SET
                issuing_authority = EXCLUDED.issuing_authority,
                is_primary = EXCLUDED.is_primary,
                source = EXCLUDED.source,
                scheme_name = EXCLUDED.scheme_name,
                uri = EXCLUDED.uri,
                updated_at = NOW()
            "#,
        )
        .bind(identifier_id)
        .bind(fields.entity_id)
        .bind(&fields.identifier_type)
        .bind(&fields.identifier_value)
        .bind(&fields.issuing_authority)
        .bind(fields.is_primary)
        .bind(&fields.source)
        .bind(&fields.scheme_name)
        .bind(&fields.uri)
        .execute(&self.pool)
        .await
        .context("Failed to attach identifier")?;

        info!(
            "Attached {} identifier {} to entity {}",
            fields.identifier_type, fields.identifier_value, fields.entity_id
        );

        Ok(identifier_id)
    }

    /// Get entity by LEI
    pub async fn get_entity_by_lei(&self, lei: &str) -> Result<Option<Uuid>> {
        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT entity_id FROM "ob-poc".entity_identifiers
            WHERE identifier_type = 'LEI' AND identifier_value = $1
            "#,
        )
        .bind(lei)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get entity by LEI")?;

        Ok(result)
    }

    /// Get entity by any identifier type
    pub async fn get_entity_by_identifier(
        &self,
        identifier_type: &str,
        identifier_value: &str,
    ) -> Result<Option<Uuid>> {
        let result = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT entity_id FROM "ob-poc".entity_identifiers
            WHERE identifier_type = $1 AND identifier_value = $2
            "#,
        )
        .bind(identifier_type)
        .bind(identifier_value)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get entity by identifier")?;

        Ok(result)
    }

    /// Get all identifiers for an entity
    pub async fn get_identifiers(&self, entity_id: Uuid) -> Result<Vec<EntityIdentifier>> {
        let results = sqlx::query_as::<_, EntityIdentifier>(
            r#"
            SELECT * FROM "ob-poc".entity_identifiers
            WHERE entity_id = $1
            ORDER BY identifier_type
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get identifiers")?;

        Ok(results)
    }

    /// Get a specific identifier type for an entity
    pub async fn get_identifier(
        &self,
        entity_id: Uuid,
        identifier_type: &str,
    ) -> Result<Option<EntityIdentifier>> {
        let result = sqlx::query_as::<_, EntityIdentifier>(
            r#"
            SELECT * FROM "ob-poc".entity_identifiers
            WHERE entity_id = $1 AND identifier_type = $2
            LIMIT 1
            "#,
        )
        .bind(entity_id)
        .bind(identifier_type)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get identifier")?;

        Ok(result)
    }

    /// Update LEI validation status (after GLEIF API call)
    pub async fn update_lei_validation(
        &self,
        entity_id: Uuid,
        lei: &str,
        lei_status: &str,
        lei_next_renewal: Option<NaiveDate>,
        managing_lou: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".entity_identifiers
            SET is_validated = true,
                validated_at = NOW(),
                validation_source = 'GLEIF_API',
                lei_status = $1,
                lei_next_renewal = $2,
                lei_managing_lou = $3,
                updated_at = NOW()
            WHERE entity_id = $4 AND identifier_type = 'LEI' AND identifier_value = $5
            "#,
        )
        .bind(lei_status)
        .bind(lei_next_renewal)
        .bind(managing_lou)
        .bind(entity_id)
        .bind(lei)
        .execute(&self.pool)
        .await
        .context("Failed to update LEI validation")?;

        Ok(result.rows_affected() > 0)
    }

    /// Mark identifier as primary for its type
    pub async fn set_primary_identifier(
        &self,
        entity_id: Uuid,
        identifier_type: &str,
        identifier_value: &str,
    ) -> Result<bool> {
        // First, unset any existing primary for this type
        sqlx::query(
            r#"
            UPDATE "ob-poc".entity_identifiers
            SET is_primary = false, updated_at = NOW()
            WHERE entity_id = $1 AND identifier_type = $2 AND is_primary = true
            "#,
        )
        .bind(entity_id)
        .bind(identifier_type)
        .execute(&self.pool)
        .await
        .context("Failed to unset existing primary")?;

        // Then set the new primary
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".entity_identifiers
            SET is_primary = true, updated_at = NOW()
            WHERE entity_id = $1 AND identifier_type = $2 AND identifier_value = $3
            "#,
        )
        .bind(entity_id)
        .bind(identifier_type)
        .bind(identifier_value)
        .execute(&self.pool)
        .await
        .context("Failed to set primary identifier")?;

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // GLEIF Relationships (Corporate Hierarchy - SEPARATE from UBO)
    // =========================================================================

    /// Create a GLEIF relationship (corporate hierarchy for accounting consolidation)
    pub async fn create_gleif_relationship(&self, fields: &NewGleifRelationship) -> Result<Uuid> {
        let gleif_rel_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".gleif_relationships
                (gleif_rel_id, parent_entity_id, parent_lei, child_entity_id, child_lei,
                 relationship_type, ownership_percentage, accounting_standard,
                 start_date, end_date, gleif_record_id, fetched_at, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW(), NOW())
            ON CONFLICT (parent_lei, child_lei, relationship_type) DO UPDATE SET
                parent_entity_id = EXCLUDED.parent_entity_id,
                child_entity_id = EXCLUDED.child_entity_id,
                ownership_percentage = EXCLUDED.ownership_percentage,
                accounting_standard = EXCLUDED.accounting_standard,
                start_date = EXCLUDED.start_date,
                end_date = EXCLUDED.end_date,
                gleif_record_id = EXCLUDED.gleif_record_id,
                fetched_at = NOW()
            "#,
        )
        .bind(gleif_rel_id)
        .bind(fields.parent_entity_id)
        .bind(&fields.parent_lei)
        .bind(fields.child_entity_id)
        .bind(&fields.child_lei)
        .bind(&fields.relationship_type)
        .bind(fields.ownership_percentage)
        .bind(&fields.accounting_standard)
        .bind(fields.start_date)
        .bind(fields.end_date)
        .bind(&fields.gleif_record_id)
        .execute(&self.pool)
        .await
        .context("Failed to create GLEIF relationship")?;

        info!(
            "Created GLEIF {} relationship: {} -> {}",
            fields.relationship_type, fields.parent_lei, fields.child_lei
        );

        Ok(gleif_rel_id)
    }

    /// Get GLEIF corporate hierarchy for an entity (parents)
    pub async fn get_gleif_parents(&self, child_lei: &str) -> Result<Vec<GleifRelationship>> {
        let results = sqlx::query_as::<_, GleifRelationship>(
            r#"
            SELECT * FROM "ob-poc".gleif_relationships
            WHERE child_lei = $1
              AND (relationship_status = 'ACTIVE' OR relationship_status IS NULL)
            ORDER BY relationship_type
            "#,
        )
        .bind(child_lei)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get GLEIF parents")?;

        Ok(results)
    }

    /// Get GLEIF corporate hierarchy for an entity (children)
    pub async fn get_gleif_children(&self, parent_lei: &str) -> Result<Vec<GleifRelationship>> {
        let results = sqlx::query_as::<_, GleifRelationship>(
            r#"
            SELECT * FROM "ob-poc".gleif_relationships
            WHERE parent_lei = $1
              AND (relationship_status = 'ACTIVE' OR relationship_status IS NULL)
            ORDER BY relationship_type
            "#,
        )
        .bind(parent_lei)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get GLEIF children")?;

        Ok(results)
    }

    /// Get GLEIF hierarchy with entity names (from view)
    pub async fn get_gleif_hierarchy(&self, lei: &str) -> Result<Vec<GleifHierarchyEntry>> {
        let results = sqlx::query_as::<_, GleifHierarchyEntry>(
            r#"
            SELECT * FROM "ob-poc".v_gleif_hierarchy
            WHERE parent_lei = $1 OR child_lei = $1
            ORDER BY relationship_type, child_name
            "#,
        )
        .bind(lei)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get GLEIF hierarchy")?;

        Ok(results)
    }

    /// Update GLEIF relationship status
    pub async fn update_gleif_relationship_status(
        &self,
        gleif_rel_id: Uuid,
        status: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".gleif_relationships
            SET relationship_status = $1, fetched_at = NOW()
            WHERE gleif_rel_id = $2
            "#,
        )
        .bind(status)
        .bind(gleif_rel_id)
        .execute(&self.pool)
        .await
        .context("Failed to update GLEIF relationship status")?;

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // BODS Interest Types (Reference Data)
    // =========================================================================

    /// Get all BODS interest types
    pub async fn get_interest_types(&self) -> Result<Vec<BodsInterestType>> {
        let results = sqlx::query_as::<_, BodsInterestType>(
            r#"
            SELECT * FROM "ob-poc".bods_interest_types
            ORDER BY display_order, type_code
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get interest types")?;

        Ok(results)
    }

    /// Get interest types by category (ownership, control, trust, beneficial)
    pub async fn get_interest_types_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<BodsInterestType>> {
        let results = sqlx::query_as::<_, BodsInterestType>(
            r#"
            SELECT * FROM "ob-poc".bods_interest_types
            WHERE category = $1
            ORDER BY display_order, type_code
            "#,
        )
        .bind(category)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get interest types by category")?;

        Ok(results)
    }

    /// Get a single interest type by code
    pub async fn get_interest_type(&self, type_code: &str) -> Result<Option<BodsInterestType>> {
        let result = sqlx::query_as::<_, BodsInterestType>(
            r#"
            SELECT * FROM "ob-poc".bods_interest_types
            WHERE type_code = $1
            "#,
        )
        .bind(type_code)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get interest type")?;

        Ok(result)
    }

    // =========================================================================
    // BODS Entity Types (Reference Data)
    // =========================================================================

    /// Get all BODS entity types
    pub async fn get_bods_entity_types(&self) -> Result<Vec<BodsEntityType>> {
        let results = sqlx::query_as::<_, BodsEntityType>(
            r#"
            SELECT * FROM "ob-poc".bods_entity_types
            ORDER BY display_order, type_code
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get BODS entity types")?;

        Ok(results)
    }

    // =========================================================================
    // Person PEP Status
    // =========================================================================

    /// Add PEP status for a person
    pub async fn add_pep_status(&self, fields: &NewPersonPepStatus) -> Result<Uuid> {
        let pep_status_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".person_pep_status
                (pep_status_id, person_entity_id, status, reason, jurisdiction,
                 position_held, position_level, start_date, end_date,
                 source_type, screening_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NOW(), NOW())
            "#,
        )
        .bind(pep_status_id)
        .bind(fields.person_entity_id)
        .bind(&fields.status)
        .bind(&fields.reason)
        .bind(&fields.jurisdiction)
        .bind(&fields.position_held)
        .bind(&fields.position_level)
        .bind(fields.start_date)
        .bind(fields.end_date)
        .bind(&fields.source_type)
        .bind(fields.screening_id)
        .execute(&self.pool)
        .await
        .context("Failed to add PEP status")?;

        info!(
            "Added PEP status {} for person {}",
            fields.status, fields.person_entity_id
        );

        Ok(pep_status_id)
    }

    /// Get all PEP status records for a person
    pub async fn get_pep_status(&self, person_entity_id: Uuid) -> Result<Vec<PersonPepStatus>> {
        let results = sqlx::query_as::<_, PersonPepStatus>(
            r#"
            SELECT * FROM "ob-poc".person_pep_status
            WHERE person_entity_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(person_entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get PEP status")?;

        Ok(results)
    }

    /// Check if person is currently a PEP
    pub async fn is_pep(&self, person_entity_id: Uuid) -> Result<bool> {
        let result = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".person_pep_status
                WHERE person_entity_id = $1
                  AND status = 'isPep'
                  AND (end_date IS NULL OR end_date > CURRENT_DATE)
            )
            "#,
        )
        .bind(person_entity_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check PEP status")?;

        Ok(result)
    }

    /// Update PEP status (e.g., when screening result comes in)
    pub async fn update_pep_status(
        &self,
        pep_status_id: Uuid,
        status: &str,
        verified_by: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".person_pep_status
            SET status = $1,
                verified_at = NOW(),
                verified_by = $2,
                updated_at = NOW()
            WHERE pep_status_id = $3
            "#,
        )
        .bind(status)
        .bind(verified_by)
        .bind(pep_status_id)
        .execute(&self.pool)
        .await
        .context("Failed to update PEP status")?;

        Ok(result.rows_affected() > 0)
    }

    /// End PEP status (person no longer holds political position)
    pub async fn end_pep_status(&self, pep_status_id: Uuid, end_date: NaiveDate) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".person_pep_status
            SET end_date = $1, updated_at = NOW()
            WHERE pep_status_id = $2
            "#,
        )
        .bind(end_date)
        .bind(pep_status_id)
        .execute(&self.pool)
        .await
        .context("Failed to end PEP status")?;

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // View Accessors
    // =========================================================================

    /// Get entities with their LEIs (from view)
    pub async fn get_entities_with_lei(&self) -> Result<Vec<EntityWithLei>> {
        let results = sqlx::query_as::<_, EntityWithLei>(
            r#"
            SELECT * FROM "ob-poc".v_entities_with_lei
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get entities with LEI")?;

        Ok(results)
    }

    /// Get entities with LEI by entity IDs
    pub async fn get_entities_with_lei_by_ids(
        &self,
        entity_ids: &[Uuid],
    ) -> Result<Vec<EntityWithLei>> {
        let results = sqlx::query_as::<_, EntityWithLei>(
            r#"
            SELECT * FROM "ob-poc".v_entities_with_lei
            WHERE entity_id = ANY($1)
            ORDER BY name
            "#,
        )
        .bind(entity_ids)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get entities with LEI by IDs")?;

        Ok(results)
    }

    /// Get UBO interests for a subject entity (from view)
    pub async fn get_ubo_interests(&self, subject_id: Uuid) -> Result<Vec<UboInterest>> {
        let results = sqlx::query_as::<_, UboInterest>(
            r#"
            SELECT * FROM "ob-poc".v_ubo_interests
            WHERE subject_id = $1
            ORDER BY interest_category, ownership_share DESC NULLS LAST
            "#,
        )
        .bind(subject_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get UBO interests")?;

        Ok(results)
    }

    /// Get UBO interests held by an interested party
    pub async fn get_interests_held_by(
        &self,
        interested_party_id: Uuid,
    ) -> Result<Vec<UboInterest>> {
        let results = sqlx::query_as::<_, UboInterest>(
            r#"
            SELECT * FROM "ob-poc".v_ubo_interests
            WHERE interested_party_id = $1
            ORDER BY interest_category, ownership_share DESC NULLS LAST
            "#,
        )
        .bind(interested_party_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get interests held by party")?;

        Ok(results)
    }
}

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
pub(crate) struct BodsService {
    pool: PgPool,
}

impl BodsService {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    // =========================================================================
    // Entity Identifiers (LEI Spine)
    // =========================================================================





    /// Get a specific identifier type for an entity
    pub(crate) async fn get_identifier(
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



    // =========================================================================
    // GLEIF Relationships (Corporate Hierarchy - SEPARATE from UBO)
    // =========================================================================






    // =========================================================================
    // BODS Interest Types (Reference Data)
    // =========================================================================

    /// Get all BODS interest types
    pub(crate) async fn get_interest_types(&self) -> Result<Vec<BodsInterestType>> {
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


    /// Get a single interest type by code
    pub(crate) async fn get_interest_type(&self, type_code: &str) -> Result<Option<BodsInterestType>> {
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


    // =========================================================================
    // Person PEP Status
    // =========================================================================






    // =========================================================================
    // View Accessors
    // =========================================================================

    /// Get entities with their LEIs (from view)
    pub(crate) async fn get_entities_with_lei(&self) -> Result<Vec<EntityWithLei>> {
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



}

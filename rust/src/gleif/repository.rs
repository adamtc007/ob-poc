//! Database operations for GLEIF data
//!
//! Handles persistence of GLEIF Level 1 and Level 2 data to our schema.

use super::types::*;
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct GleifRepository {
    pub(crate) pool: Arc<PgPool>,
}

impl GleifRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Update entity with GLEIF data
    pub async fn update_entity_from_gleif(
        &self,
        entity_id: Uuid,
        record: &LeiRecord,
    ) -> Result<()> {
        let entity = &record.attributes.entity;
        let reg = &record.attributes.registration;

        // Parse dates
        let last_update: Option<DateTime<Utc>> = reg
            .last_update_date
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let next_renewal: Option<NaiveDate> = reg
            .next_renewal_date
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let creation_date: Option<NaiveDate> = entity
            .creation_date
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        sqlx::query(
            r#"
            UPDATE "ob-poc".entity_limited_companies
            SET lei = $2,
                gleif_status = $3,
                gleif_category = $4,
                gleif_subcategory = $5,
                legal_form_code = $6,
                legal_form_text = $7,
                gleif_validation_level = $8,
                gleif_last_update = $9,
                gleif_next_renewal = $10,
                entity_creation_date = $11,
                headquarters_address = $12,
                headquarters_city = $13,
                headquarters_country = $14,
                updated_at = NOW()
            WHERE entity_id = $1
        "#,
        )
        .bind(entity_id)
        .bind(&record.attributes.lei)
        .bind(&entity.status)
        .bind(&entity.category)
        .bind(&entity.sub_category)
        .bind(entity.legal_form.as_ref().and_then(|lf| lf.id.clone()))
        .bind(entity.legal_form.as_ref().and_then(|lf| lf.other.clone()))
        .bind(&reg.corroboration_level)
        .bind(last_update)
        .bind(next_renewal)
        .bind(creation_date)
        .bind(
            entity
                .headquarters_address
                .as_ref()
                .map(|a| a.address_lines.join(", ")),
        )
        .bind(
            entity
                .headquarters_address
                .as_ref()
                .and_then(|a| a.city.clone()),
        )
        .bind(
            entity
                .headquarters_address
                .as_ref()
                .and_then(|a| a.country.clone()),
        )
        .execute(self.pool.as_ref())
        .await
        .context("Failed to update entity with GLEIF data")?;

        Ok(())
    }

    /// Insert alternative names
    pub async fn insert_entity_names(&self, entity_id: Uuid, record: &LeiRecord) -> Result<i32> {
        let entity = &record.attributes.entity;
        let mut count = 0;

        // Clear existing names from GLEIF source
        sqlx::query(
            r#"
            DELETE FROM "ob-poc".entity_names
            WHERE entity_id = $1 AND source = 'GLEIF'
        "#,
        )
        .bind(entity_id)
        .execute(self.pool.as_ref())
        .await?;

        // Insert legal name
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_names
                (entity_id, name_type, name, language, is_primary, source)
            VALUES ($1, 'LEGAL', $2, $3, true, 'GLEIF')
        "#,
        )
        .bind(entity_id)
        .bind(&entity.legal_name.name)
        .bind(&entity.legal_name.language)
        .execute(self.pool.as_ref())
        .await?;
        count += 1;

        // Insert other names
        for other in &entity.other_names {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".entity_names
                    (entity_id, name_type, name, language, is_primary, source)
                VALUES ($1, $2, $3, $4, false, 'GLEIF')
            "#,
            )
            .bind(entity_id)
            .bind(other.name_type.as_deref().unwrap_or("ALTERNATIVE"))
            .bind(&other.name)
            .bind(&other.language)
            .execute(self.pool.as_ref())
            .await?;
            count += 1;
        }

        // Insert transliterated names
        for trans in &entity.transliterated_other_names {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".entity_names
                    (entity_id, name_type, name, language, is_primary, source)
                VALUES ($1, 'TRANSLITERATED', $2, $3, false, 'GLEIF')
            "#,
            )
            .bind(entity_id)
            .bind(&trans.name)
            .bind(&trans.language)
            .execute(self.pool.as_ref())
            .await?;
            count += 1;
        }

        Ok(count)
    }

    /// Insert addresses
    pub async fn insert_entity_addresses(
        &self,
        entity_id: Uuid,
        record: &LeiRecord,
    ) -> Result<i32> {
        let entity = &record.attributes.entity;
        let mut count = 0;

        // Clear existing GLEIF addresses
        sqlx::query(
            r#"
            DELETE FROM "ob-poc".entity_addresses
            WHERE entity_id = $1 AND source = 'GLEIF'
        "#,
        )
        .bind(entity_id)
        .execute(self.pool.as_ref())
        .await?;

        // Insert legal address
        self.insert_address(entity_id, "LEGAL", &entity.legal_address, true)
            .await?;
        count += 1;

        // Insert HQ address
        if let Some(hq) = &entity.headquarters_address {
            self.insert_address(entity_id, "HEADQUARTERS", hq, false)
                .await?;
            count += 1;
        }

        // Insert other addresses
        for other in &entity.other_addresses {
            self.insert_address(entity_id, &other.address_type, &other.address, false)
                .await?;
            count += 1;
        }

        Ok(count)
    }

    async fn insert_address(
        &self,
        entity_id: Uuid,
        address_type: &str,
        address: &Address,
        is_primary: bool,
    ) -> Result<()> {
        let country = address.country.as_deref().unwrap_or("XX");

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_addresses
                (entity_id, address_type, language, address_lines, city, region,
                 country, postal_code, is_primary, source)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'GLEIF')
        "#,
        )
        .bind(entity_id)
        .bind(address_type)
        .bind(&address.language)
        .bind(&address.address_lines)
        .bind(&address.city)
        .bind(&address.region)
        .bind(country)
        .bind(&address.postal_code)
        .bind(is_primary)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Insert identifiers (LEI, BIC, etc.)
    pub async fn insert_entity_identifiers(
        &self,
        entity_id: Uuid,
        record: &LeiRecord,
        bics: &[BicMapping],
    ) -> Result<i32> {
        let mut count = 0;

        // Clear existing GLEIF identifiers
        sqlx::query(
            r#"
            DELETE FROM "ob-poc".entity_identifiers
            WHERE entity_id = $1 AND source = 'GLEIF'
        "#,
        )
        .bind(entity_id)
        .execute(self.pool.as_ref())
        .await?;

        // Insert LEI
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_identifiers
                (entity_id, identifier_type, identifier_value, is_primary, source)
            VALUES ($1, 'LEI', $2, true, 'GLEIF')
            ON CONFLICT (entity_id, identifier_type, identifier_value) DO NOTHING
        "#,
        )
        .bind(entity_id)
        .bind(&record.attributes.lei)
        .execute(self.pool.as_ref())
        .await?;
        count += 1;

        // Insert registration number
        if let Some(reg_num) = &record.attributes.entity.registered_as {
            let authority = record
                .attributes
                .entity
                .registered_at
                .as_ref()
                .and_then(|ra| ra.id.clone().or(ra.other.clone()));

            sqlx::query(
                r#"
                INSERT INTO "ob-poc".entity_identifiers
                    (entity_id, identifier_type, identifier_value, issuing_authority, source)
                VALUES ($1, 'REG_NUM', $2, $3, 'GLEIF')
                ON CONFLICT (entity_id, identifier_type, identifier_value) DO NOTHING
            "#,
            )
            .bind(entity_id)
            .bind(reg_num)
            .bind(authority)
            .execute(self.pool.as_ref())
            .await?;
            count += 1;
        }

        // Insert BICs
        for bic in bics {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".entity_identifiers
                    (entity_id, identifier_type, identifier_value, issuing_authority, source)
                VALUES ($1, 'BIC', $2, 'SWIFT', 'GLEIF')
                ON CONFLICT (entity_id, identifier_type, identifier_value) DO NOTHING
            "#,
            )
            .bind(entity_id)
            .bind(&bic.attributes.bic)
            .execute(self.pool.as_ref())
            .await?;
            count += 1;
        }

        Ok(count)
    }

    /// Insert parent relationships
    pub async fn insert_parent_relationship(
        &self,
        child_entity_id: Uuid,
        parent_lei: &str,
        parent_name: Option<&str>,
        relationship_type: &str,
        accounting_standard: Option<&str>,
    ) -> Result<()> {
        // Try to find parent entity in our system
        let parent_entity_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT e.entity_id
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_limited_companies c ON c.entity_id = e.entity_id
            WHERE c.lei = $1
        "#,
        )
        .bind(parent_lei)
        .fetch_optional(self.pool.as_ref())
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_parent_relationships
                (child_entity_id, parent_entity_id, parent_lei, parent_name,
                 relationship_type, accounting_standard, source)
            VALUES ($1, $2, $3, $4, $5, $6, 'GLEIF')
            ON CONFLICT (child_entity_id, parent_lei, relationship_type)
            DO UPDATE SET
                parent_entity_id = EXCLUDED.parent_entity_id,
                parent_name = EXCLUDED.parent_name,
                updated_at = NOW()
        "#,
        )
        .bind(child_entity_id)
        .bind(parent_entity_id)
        .bind(parent_lei)
        .bind(parent_name)
        .bind(relationship_type)
        .bind(accounting_standard)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Insert lifecycle events
    pub async fn insert_lifecycle_events(
        &self,
        entity_id: Uuid,
        record: &LeiRecord,
    ) -> Result<i32> {
        let mut count = 0;

        for event_group in &record.attributes.entity.event_groups {
            for event in &event_group.events {
                let effective_date: Option<NaiveDate> = event
                    .effective_date
                    .as_ref()
                    .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

                let recorded_date: Option<NaiveDate> = event
                    .recorded_date
                    .as_ref()
                    .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

                // Build affected fields as JSONB
                let affected_fields: serde_json::Value = event
                    .affected_fields
                    .iter()
                    .map(|f| {
                        serde_json::json!({
                            "xpath": f.xpath,
                            "value": f.value
                        })
                    })
                    .collect();

                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".entity_lifecycle_events
                        (entity_id, event_type, event_status, effective_date, recorded_date,
                         affected_fields, validation_documents, validation_reference, source)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'GLEIF')
                "#,
                )
                .bind(entity_id)
                .bind(&event.event_type)
                .bind(&event.status)
                .bind(effective_date)
                .bind(recorded_date)
                .bind(&affected_fields)
                .bind(&event.validation_documents)
                .bind(&event.validation_reference)
                .execute(self.pool.as_ref())
                .await?;

                count += 1;
            }
        }

        Ok(count)
    }

    /// Update entity with parent exception codes
    pub async fn update_parent_exceptions(
        &self,
        entity_id: Uuid,
        direct_exception: Option<&str>,
        ultimate_exception: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".entity_limited_companies
            SET gleif_direct_parent_exception = $2,
                gleif_ultimate_parent_exception = $3,
                updated_at = NOW()
            WHERE entity_id = $1
        "#,
        )
        .bind(entity_id)
        .bind(direct_exception)
        .bind(ultimate_exception)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Update UBO status for an entity
    pub async fn update_ubo_status(&self, entity_id: Uuid, status: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".entity_limited_companies
            SET ubo_status = $2,
                updated_at = NOW()
            WHERE entity_id = $1
        "#,
        )
        .bind(entity_id)
        .bind(status)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Log a sync operation
    pub async fn log_sync(
        &self,
        entity_id: Option<Uuid>,
        lei: Option<&str>,
        sync_type: &str,
        sync_status: &str,
        records_fetched: i32,
        records_updated: i32,
        records_created: i32,
        error_message: Option<&str>,
    ) -> Result<Uuid> {
        let sync_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".gleif_sync_log
                (entity_id, lei, sync_type, sync_status, records_fetched,
                 records_updated, records_created, error_message, completed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                    CASE WHEN $4 != 'IN_PROGRESS' THEN NOW() ELSE NULL END)
            RETURNING sync_id
        "#,
        )
        .bind(entity_id)
        .bind(lei)
        .bind(sync_type)
        .bind(sync_status)
        .bind(records_fetched)
        .bind(records_updated)
        .bind(records_created)
        .bind(error_message)
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(sync_id)
    }

    /// Find entity by LEI
    pub async fn find_entity_by_lei(&self, lei: &str) -> Result<Option<Uuid>> {
        let entity_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT entity_id FROM "ob-poc".entity_limited_companies
            WHERE lei = $1
        "#,
        )
        .bind(lei)
        .fetch_optional(self.pool.as_ref())
        .await?;

        Ok(entity_id)
    }

    /// Create a new entity from GLEIF data
    pub async fn create_entity_from_gleif(&self, record: &LeiRecord) -> Result<Uuid> {
        let entity = &record.attributes.entity;

        // First create the base entity
        let entity_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".entities (entity_type_id, name)
            SELECT entity_type_id, $1
            FROM "ob-poc".entity_types
            WHERE type_code = 'limited_company'
            RETURNING entity_id
        "#,
        )
        .bind(&entity.legal_name.name)
        .fetch_one(self.pool.as_ref())
        .await?;

        // Then create the limited company extension
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_limited_companies
                (entity_id, company_name, jurisdiction, lei)
            VALUES ($1, $2, $3, $4)
        "#,
        )
        .bind(entity_id)
        .bind(&entity.legal_name.name)
        .bind(&entity.jurisdiction)
        .bind(&record.attributes.lei)
        .execute(self.pool.as_ref())
        .await?;

        Ok(entity_id)
    }
}

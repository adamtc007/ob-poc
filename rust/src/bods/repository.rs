//! BODS Repository - Database operations for BODS data

use anyhow::Result;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use super::types::*;

/// Repository for BODS database operations
pub struct BodsRepository {
    pub(crate) pool: Arc<PgPool>,
}

impl BodsRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // ========================================================================
    // BODS Entity Statements
    // ========================================================================

    /// Insert or update a BODS entity statement
    pub async fn upsert_entity_statement(&self, stmt: &EntityStatement) -> Result<()> {
        let lei = stmt
            .identifiers
            .iter()
            .find_map(|id| id.as_lei())
            .map(String::from);
        let company_number = stmt
            .identifiers
            .iter()
            .find_map(|id| id.as_company_number())
            .map(String::from);
        let jurisdiction = stmt.jurisdiction.as_ref().and_then(|j| j.code.clone());
        let statement_date = stmt
            .statement_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        let source_url = stmt.source.as_ref().and_then(|s| s.url.clone());
        let source_register = stmt.source.as_ref().and_then(|s| s.description.clone());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".bods_entity_statements (
                statement_id, entity_type, name, jurisdiction,
                lei, company_number, opencorporates_id, identifiers,
                source_register, statement_date, source_url
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (statement_id) DO UPDATE SET
                entity_type = EXCLUDED.entity_type,
                name = EXCLUDED.name,
                jurisdiction = EXCLUDED.jurisdiction,
                lei = EXCLUDED.lei,
                company_number = EXCLUDED.company_number,
                identifiers = EXCLUDED.identifiers,
                source_register = EXCLUDED.source_register,
                statement_date = EXCLUDED.statement_date,
                source_url = EXCLUDED.source_url
        "#,
        )
        .bind(&stmt.statement_id)
        .bind(stmt.entity_type.as_str())
        .bind(&stmt.name)
        .bind(&jurisdiction)
        .bind(&lei)
        .bind(&company_number)
        .bind::<Option<&str>>(None) // opencorporates_id
        .bind(serde_json::to_value(&stmt.identifiers).ok())
        .bind(&source_register)
        .bind(statement_date)
        .bind(&source_url)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Find entity statement by LEI
    pub async fn find_entity_by_lei(&self, lei: &str) -> Result<Option<BodsEntityRow>> {
        let row = sqlx::query_as::<_, BodsEntityRow>(
            r#"
            SELECT * FROM "ob-poc".bods_entity_statements
            WHERE lei = $1
        "#,
        )
        .bind(lei)
        .fetch_optional(self.pool.as_ref())
        .await?;

        Ok(row)
    }

    /// Find entity statement by company number
    pub async fn find_entity_by_company_number(
        &self,
        number: &str,
    ) -> Result<Option<BodsEntityRow>> {
        let row = sqlx::query_as::<_, BodsEntityRow>(
            r#"
            SELECT * FROM "ob-poc".bods_entity_statements
            WHERE company_number = $1
        "#,
        )
        .bind(number)
        .fetch_optional(self.pool.as_ref())
        .await?;

        Ok(row)
    }

    // ========================================================================
    // BODS Person Statements
    // ========================================================================

    /// Insert or update a BODS person statement
    pub async fn upsert_person_statement(&self, stmt: &PersonStatement) -> Result<()> {
        let primary_name = stmt.names.first();
        let full_name = primary_name.and_then(|n| n.full_name.clone());
        let given_name = primary_name.and_then(|n| n.given_name.clone());
        let family_name = primary_name.and_then(|n| n.family_name.clone());

        let birth_date = stmt.birth_date.as_ref().and_then(|d| {
            // Try exact date first
            NaiveDate::parse_from_str(d, "%Y-%m-%d").ok().or_else(|| {
                // Try year-month
                if d.len() == 7 {
                    NaiveDate::parse_from_str(&format!("{}-01", d), "%Y-%m-%d").ok()
                } else if d.len() == 4 {
                    // Just year
                    NaiveDate::parse_from_str(&format!("{}-01-01", d), "%Y-%m-%d").ok()
                } else {
                    None
                }
            })
        });

        let birth_date_precision = stmt.birth_date.as_ref().map(|d| {
            if d.len() == 10 {
                "exact"
            } else if d.len() == 7 {
                "month"
            } else {
                "year"
            }
        });

        let nationalities: Vec<String> = stmt
            .nationalities
            .iter()
            .filter_map(|j| j.code.clone())
            .collect();

        let country_of_residence = stmt.addresses.first().and_then(|a| a.country.clone());

        let statement_date = stmt
            .statement_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let source_register = stmt.source.as_ref().and_then(|s| s.description.clone());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".bods_person_statements (
                statement_id, person_type, full_name, given_name, family_name,
                names, birth_date, birth_date_precision, nationalities,
                country_of_residence, addresses, source_register, statement_date
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (statement_id) DO UPDATE SET
                person_type = EXCLUDED.person_type,
                full_name = EXCLUDED.full_name,
                given_name = EXCLUDED.given_name,
                family_name = EXCLUDED.family_name,
                names = EXCLUDED.names,
                birth_date = EXCLUDED.birth_date,
                birth_date_precision = EXCLUDED.birth_date_precision,
                nationalities = EXCLUDED.nationalities,
                country_of_residence = EXCLUDED.country_of_residence,
                addresses = EXCLUDED.addresses,
                source_register = EXCLUDED.source_register,
                statement_date = EXCLUDED.statement_date
        "#,
        )
        .bind(&stmt.statement_id)
        .bind(stmt.person_type.as_str())
        .bind(&full_name)
        .bind(&given_name)
        .bind(&family_name)
        .bind(serde_json::to_value(&stmt.names).ok())
        .bind(birth_date)
        .bind(birth_date_precision)
        .bind(&nationalities)
        .bind(&country_of_residence)
        .bind(serde_json::to_value(&stmt.addresses).ok())
        .bind(&source_register)
        .bind(statement_date)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Find person statement by ID
    pub async fn find_person_by_id(&self, statement_id: &str) -> Result<Option<BodsPersonRow>> {
        let row = sqlx::query_as::<_, BodsPersonRow>(
            r#"
            SELECT * FROM "ob-poc".bods_person_statements
            WHERE statement_id = $1
        "#,
        )
        .bind(statement_id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        Ok(row)
    }

    // ========================================================================
    // BODS Ownership Statements
    // ========================================================================

    /// Insert or update a BODS ownership statement
    pub async fn upsert_ownership_statement(&self, stmt: &OwnershipStatement) -> Result<()> {
        let interested_party_type = if stmt.interested_party.person_statement_id.is_some() {
            Some("person")
        } else if stmt.interested_party.entity_statement_id.is_some() {
            Some("entity")
        } else {
            None
        };

        let interested_party_statement_id = stmt
            .interested_party
            .person_statement_id
            .as_ref()
            .or(stmt.interested_party.entity_statement_id.as_ref());

        let first_interest = stmt.interests.first();
        let ownership_type = first_interest.and_then(|i| i.interest_type.clone());
        let is_direct = first_interest.map(|i| i.interest_level.as_deref() == Some("direct"));

        let share_min = first_interest
            .and_then(|i| i.share.as_ref())
            .and_then(|s| s.minimum.and_then(|v| Decimal::try_from(v).ok()));
        let share_max = first_interest
            .and_then(|i| i.share.as_ref())
            .and_then(|s| s.maximum.and_then(|v| Decimal::try_from(v).ok()));
        let share_exact = first_interest
            .and_then(|i| i.share.as_ref())
            .and_then(|s| s.exact.and_then(|v| Decimal::try_from(v).ok()));

        let start_date = first_interest
            .and_then(|i| i.start_date.as_ref())
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        let end_date = first_interest
            .and_then(|i| i.end_date.as_ref())
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let statement_date = stmt
            .statement_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let source_register = stmt.source.as_ref().and_then(|s| s.description.clone());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".bods_ownership_statements (
                statement_id, subject_entity_statement_id,
                interested_party_type, interested_party_statement_id,
                ownership_type, share_min, share_max, share_exact, is_direct,
                start_date, end_date, source_register, statement_date, source_description
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (statement_id) DO UPDATE SET
                subject_entity_statement_id = EXCLUDED.subject_entity_statement_id,
                interested_party_type = EXCLUDED.interested_party_type,
                interested_party_statement_id = EXCLUDED.interested_party_statement_id,
                ownership_type = EXCLUDED.ownership_type,
                share_min = EXCLUDED.share_min,
                share_max = EXCLUDED.share_max,
                share_exact = EXCLUDED.share_exact,
                is_direct = EXCLUDED.is_direct,
                start_date = EXCLUDED.start_date,
                end_date = EXCLUDED.end_date,
                source_register = EXCLUDED.source_register,
                statement_date = EXCLUDED.statement_date,
                source_description = EXCLUDED.source_description
        "#,
        )
        .bind(&stmt.statement_id)
        .bind(stmt.subject.entity_statement_id.as_ref())
        .bind(interested_party_type)
        .bind(interested_party_statement_id)
        .bind(&ownership_type)
        .bind(share_min)
        .bind(share_max)
        .bind(share_exact)
        .bind(is_direct)
        .bind(start_date)
        .bind(end_date)
        .bind(&source_register)
        .bind(statement_date)
        .bind::<Option<&str>>(None) // source_description
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    /// Find ownership statements for a subject entity
    pub async fn find_ownership_by_subject(
        &self,
        entity_statement_id: &str,
    ) -> Result<Vec<BodsOwnershipRow>> {
        let rows = sqlx::query_as::<_, BodsOwnershipRow>(
            r#"
            SELECT * FROM "ob-poc".bods_ownership_statements
            WHERE subject_entity_statement_id = $1
        "#,
        )
        .bind(entity_statement_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows)
    }

    /// Find ownership statements by subject LEI
    pub async fn find_ownership_by_subject_lei(&self, lei: &str) -> Result<Vec<BodsOwnershipRow>> {
        let rows = sqlx::query_as::<_, BodsOwnershipRow>(
            r#"
            SELECT * FROM "ob-poc".bods_ownership_statements
            WHERE subject_lei = $1
        "#,
        )
        .bind(lei)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows)
    }

    // ========================================================================
    // Entity-BODS Links
    // ========================================================================

    /// Link our entity to a BODS entity statement
    pub async fn link_entity_to_bods(
        &self,
        entity_id: Uuid,
        bods_statement_id: &str,
        match_method: &str,
        confidence: Option<f64>,
    ) -> Result<Uuid> {
        let link_id = Uuid::new_v4();
        let confidence_decimal = confidence.and_then(|c| Decimal::try_from(c).ok());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_bods_links (
                link_id, entity_id, bods_entity_statement_id, match_method, match_confidence
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (entity_id, bods_entity_statement_id) DO UPDATE SET
                match_method = EXCLUDED.match_method,
                match_confidence = EXCLUDED.match_confidence
        "#,
        )
        .bind(link_id)
        .bind(entity_id)
        .bind(bods_statement_id)
        .bind(match_method)
        .bind(confidence_decimal)
        .execute(self.pool.as_ref())
        .await?;

        Ok(link_id)
    }

    /// Find BODS links for an entity
    pub async fn find_bods_links_for_entity(
        &self,
        entity_id: Uuid,
    ) -> Result<Vec<EntityBodsLinkRow>> {
        let rows = sqlx::query_as::<_, EntityBodsLinkRow>(
            r#"
            SELECT * FROM "ob-poc".entity_bods_links
            WHERE entity_id = $1
        "#,
        )
        .bind(entity_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows)
    }

    // ========================================================================
    // Entity UBOs (Summary Table)
    // ========================================================================

    /// Insert or update a discovered UBO
    pub async fn upsert_entity_ubo(&self, ubo: &DiscoveredUbo, entity_id: Uuid) -> Result<Uuid> {
        let ubo_id = Uuid::new_v4();
        let ownership_chain = serde_json::to_value(&ubo.ownership_chain).ok();
        let chain_depth = ubo.ownership_chain.len() as i32;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_ubos (
                ubo_id, entity_id, person_statement_id, person_name,
                nationalities, country_of_residence, ownership_chain, chain_depth,
                ownership_min, ownership_max, ownership_exact, control_types, is_direct,
                ubo_type, confidence_level, source
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (ubo_id) DO UPDATE SET
                person_name = EXCLUDED.person_name,
                nationalities = EXCLUDED.nationalities,
                ownership_chain = EXCLUDED.ownership_chain,
                chain_depth = EXCLUDED.chain_depth,
                ownership_min = EXCLUDED.ownership_min,
                ownership_max = EXCLUDED.ownership_max,
                ownership_exact = EXCLUDED.ownership_exact,
                control_types = EXCLUDED.control_types,
                ubo_type = EXCLUDED.ubo_type,
                confidence_level = EXCLUDED.confidence_level,
                source = EXCLUDED.source
        "#,
        )
        .bind(ubo_id)
        .bind(entity_id)
        .bind(&ubo.person_statement_id)
        .bind(&ubo.name)
        .bind(&ubo.nationalities)
        .bind(&ubo.country_of_residence)
        .bind(&ownership_chain)
        .bind(chain_depth)
        .bind(ubo.ownership_min.and_then(|v| Decimal::try_from(v).ok()))
        .bind(ubo.ownership_max.and_then(|v| Decimal::try_from(v).ok()))
        .bind(
            ubo.ownership_percentage
                .and_then(|v| Decimal::try_from(v).ok()),
        )
        .bind(&ubo.control_types)
        .bind(ubo.is_direct)
        .bind(ubo.ubo_type.as_str())
        .bind(if ubo.confidence >= 0.9 {
            "HIGH"
        } else if ubo.confidence >= 0.7 {
            "MEDIUM"
        } else {
            "LOW"
        })
        .bind(&ubo.source)
        .execute(self.pool.as_ref())
        .await?;

        Ok(ubo_id)
    }

    /// Find UBOs for an entity
    pub async fn find_ubos_for_entity(&self, entity_id: Uuid) -> Result<Vec<EntityUboRow>> {
        let rows = sqlx::query_as::<_, EntityUboRow>(
            r#"
            SELECT * FROM "ob-poc".entity_ubos
            WHERE entity_id = $1
            ORDER BY ownership_exact DESC NULLS LAST, person_name
        "#,
        )
        .bind(entity_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(rows)
    }

    /// Delete existing UBOs for an entity (before re-discovery)
    pub async fn delete_ubos_for_entity(&self, entity_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".entity_ubos WHERE entity_id = $1
        "#,
        )
        .bind(entity_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected())
    }
}

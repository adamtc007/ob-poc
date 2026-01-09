//! UBO Discovery Service
//! Combines GLEIF corporate traversal with BODS person lookup

use super::repository::BodsRepository;
use super::types::*;
use anyhow::Result;
use chrono::NaiveDate;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Service for discovering Ultimate Beneficial Owners
pub struct UboDiscoveryService {
    pool: Arc<PgPool>,
    bods_repo: BodsRepository,
}

/// Result of UBO discovery
#[derive(Debug, Clone)]
pub enum UboResult {
    /// Natural person(s) identified as UBO
    NaturalPersons(Vec<DiscoveredUbo>),

    /// Publicly traded / widely held - no single UBO
    PublicFloat {
        terminal_lei: String,
        terminal_name: String,
    },

    /// State-owned entity
    StateOwned { state_name: String },

    /// Unknown - needs manual investigation
    Unknown { reason: String },
}

impl UboDiscoveryService {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            pool: pool.clone(),
            bods_repo: BodsRepository::new(pool),
        }
    }

    /// Discover UBOs for an entity, combining GLEIF and BODS data
    pub async fn discover_ubos(&self, entity_id: Uuid) -> Result<UboDiscoveryResult> {
        // 1. Get entity's LEI
        let lei: Option<String> = sqlx::query_scalar(
            r#"
            SELECT lei FROM "ob-poc".entity_limited_companies
            WHERE entity_id = $1
        "#,
        )
        .bind(entity_id)
        .fetch_optional(self.pool.as_ref())
        .await?
        .flatten();

        let lei = match lei {
            Some(l) => l,
            None => {
                return Ok(UboDiscoveryResult {
                    entity_id,
                    entity_lei: None,
                    ubos: vec![],
                    is_complete: false,
                    gaps: vec!["Entity has no LEI - cannot query GLEIF/BODS".to_string()],
                    sources_queried: vec![],
                });
            }
        };

        // 2. Check GLEIF reporting exceptions
        let exceptions: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            r#"
                SELECT gleif_direct_parent_exception, gleif_ultimate_parent_exception
                FROM "ob-poc".entity_limited_companies
                WHERE lei = $1
            "#,
        )
        .bind(&lei)
        .fetch_optional(self.pool.as_ref())
        .await?;

        let (direct_exception, ultimate_exception) = exceptions.unwrap_or((None, None));

        let mut sources_queried = vec!["GLEIF".to_string()];
        let mut gaps = vec![];

        // 3. Handle exceptions
        match ultimate_exception.as_deref() {
            Some("NO_KNOWN_PERSON") => {
                // Publicly traded / widely held
                let name = self.get_entity_name(&lei).await?;
                return Ok(UboDiscoveryResult {
                    entity_id,
                    entity_lei: Some(lei.clone()),
                    ubos: vec![DiscoveredUbo {
                        person_statement_id: None,
                        name: format!("Public Float ({})", name),
                        nationalities: vec![],
                        country_of_residence: None,
                        birth_date: None,
                        ownership_percentage: None,
                        ownership_min: None,
                        ownership_max: None,
                        is_direct: false,
                        control_types: vec![],
                        ownership_chain: vec![ChainLink {
                            entity_name: name,
                            entity_lei: Some(lei.clone()),
                            ownership_percentage: None,
                            relationship_type: "PUBLIC_FLOAT".to_string(),
                        }],
                        ubo_type: UboType::PublicFloat,
                        confidence: 0.95,
                        source: "GLEIF".to_string(),
                    }],
                    is_complete: true,
                    gaps: vec![],
                    sources_queried,
                });
            }
            Some("NATURAL_PERSONS") => {
                // Human owners - query BODS
                sources_queried.push("BODS".to_string());
                return self
                    .query_bods_for_ubos(entity_id, &lei, sources_queried)
                    .await;
            }
            Some("NON_CONSOLIDATING") | Some("NO_LEI") => {
                gaps.push(format!("GLEIF exception: {}", ultimate_exception.unwrap()));
            }
            _ => {}
        }

        // Check direct parent exception too
        if let Some(exc) = direct_exception.as_deref() {
            if exc == "NATURAL_PERSONS" {
                sources_queried.push("BODS".to_string());
                return self
                    .query_bods_for_ubos(entity_id, &lei, sources_queried)
                    .await;
            }
        }

        // 4. If no exception, traverse to ultimate parent
        let ultimate_parent_lei = self.get_ultimate_parent(&lei).await?;

        match ultimate_parent_lei {
            Some(parent_lei) => {
                // Check BODS for ultimate parent
                sources_queried.push("BODS".to_string());
                self.query_bods_for_ubos(entity_id, &parent_lei, sources_queried)
                    .await
            }
            None => {
                // No parent found - check if this entity has BODS data
                sources_queried.push("BODS".to_string());
                self.query_bods_for_ubos(entity_id, &lei, sources_queried)
                    .await
            }
        }
    }

    /// Query BODS for person statements linked to an LEI
    async fn query_bods_for_ubos(
        &self,
        entity_id: Uuid,
        lei: &str,
        sources_queried: Vec<String>,
    ) -> Result<UboDiscoveryResult> {
        // Find BODS entity statement by LEI
        let entity_statement = self.bods_repo.find_entity_by_lei(lei).await?;

        let entity_stmt_id = match entity_statement {
            Some(stmt) => stmt.statement_id,
            None => {
                return Ok(UboDiscoveryResult {
                    entity_id,
                    entity_lei: Some(lei.to_string()),
                    ubos: vec![],
                    is_complete: false,
                    gaps: vec![format!("No BODS data for LEI {}", lei)],
                    sources_queried,
                });
            }
        };

        // Find ownership statements where this entity is the subject
        #[derive(sqlx::FromRow)]
        struct UboRow {
            statement_id: String,
            full_name: Option<String>,
            nationalities: Option<Vec<String>>,
            country_of_residence: Option<String>,
            birth_date: Option<NaiveDate>,
            share_exact: Option<rust_decimal::Decimal>,
            share_min: Option<rust_decimal::Decimal>,
            share_max: Option<rust_decimal::Decimal>,
            control_types: Option<Vec<String>>,
            is_direct: Option<bool>,
        }

        let ubo_rows: Vec<UboRow> = sqlx::query_as(
            r#"
            SELECT
                p.statement_id,
                p.full_name,
                p.nationalities,
                p.country_of_residence,
                p.birth_date,
                o.share_exact,
                o.share_min,
                o.share_max,
                o.control_types,
                o.is_direct
            FROM "ob-poc".bods_ownership_statements o
            JOIN "ob-poc".bods_person_statements p
                ON p.statement_id = o.interested_party_statement_id
            WHERE o.subject_entity_statement_id = $1
              AND o.interested_party_type = 'person'
              AND (o.end_date IS NULL OR o.end_date > CURRENT_DATE)
        "#,
        )
        .bind(&entity_stmt_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        let ubos: Vec<DiscoveredUbo> = ubo_rows
            .into_iter()
            .map(|row| {
                DiscoveredUbo {
                    person_statement_id: Some(row.statement_id),
                    name: row.full_name.unwrap_or_else(|| "Unknown".to_string()),
                    nationalities: row.nationalities.unwrap_or_default(),
                    country_of_residence: row.country_of_residence,
                    birth_date: row.birth_date,
                    ownership_percentage: row
                        .share_exact
                        .map(|d| d.to_string().parse().unwrap_or(0.0)),
                    ownership_min: row.share_min.map(|d| d.to_string().parse().unwrap_or(0.0)),
                    ownership_max: row.share_max.map(|d| d.to_string().parse().unwrap_or(0.0)),
                    is_direct: row.is_direct.unwrap_or(true),
                    control_types: row.control_types.unwrap_or_default(),
                    ownership_chain: vec![], // TODO: Build full chain from entity_parent_relationships
                    ubo_type: UboType::NaturalPerson,
                    confidence: 0.85,
                    source: "BODS".to_string(),
                }
            })
            .collect();

        let is_complete = !ubos.is_empty();
        let gaps = if ubos.is_empty() {
            vec![format!(
                "BODS data exists but no person statements for LEI {}",
                lei
            )]
        } else {
            vec![]
        };

        Ok(UboDiscoveryResult {
            entity_id,
            entity_lei: Some(lei.to_string()),
            ubos,
            is_complete,
            gaps,
            sources_queried,
        })
    }

    /// Get entity name by LEI
    async fn get_entity_name(&self, lei: &str) -> Result<String> {
        let name: Option<String> = sqlx::query_scalar(
            r#"
            SELECT company_name FROM "ob-poc".entity_limited_companies
            WHERE lei = $1
        "#,
        )
        .bind(lei)
        .fetch_optional(self.pool.as_ref())
        .await?
        .flatten();

        Ok(name.unwrap_or_else(|| "Unknown".to_string()))
    }

    /// Get ultimate parent LEI
    async fn get_ultimate_parent(&self, lei: &str) -> Result<Option<String>> {
        let result: Option<Option<String>> = sqlx::query_scalar(
            r#"
            SELECT ultimate_parent_lei FROM "ob-poc".entity_limited_companies
            WHERE lei = $1
        "#,
        )
        .bind(lei)
        .fetch_optional(self.pool.as_ref())
        .await?;

        Ok(result.flatten())
    }

    /// Save discovered UBOs to the database
    pub async fn save_discovery_result(&self, result: &UboDiscoveryResult) -> Result<Vec<Uuid>> {
        // Delete existing UBOs for this entity
        self.bods_repo
            .delete_ubos_for_entity(result.entity_id)
            .await?;

        // Insert new UBOs
        let mut ubo_ids = vec![];
        for ubo in &result.ubos {
            let ubo_id = self
                .bods_repo
                .upsert_entity_ubo(ubo, result.entity_id)
                .await?;
            ubo_ids.push(ubo_id);
        }

        Ok(ubo_ids)
    }

    /// Discover and save UBOs for an entity
    pub async fn discover_and_save(&self, entity_id: Uuid) -> Result<UboDiscoveryResult> {
        let result = self.discover_ubos(entity_id).await?;
        self.save_discovery_result(&result).await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ubo_type_conversion() {
        assert_eq!(UboType::NaturalPerson.as_str(), "NATURAL_PERSON");
        assert_eq!(UboType::parse("PUBLIC_FLOAT"), UboType::PublicFloat);
        assert_eq!(UboType::parse("unknown_value"), UboType::Unknown);
    }
}

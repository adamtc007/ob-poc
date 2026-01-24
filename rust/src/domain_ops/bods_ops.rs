//! BODS custom operations
//!
//! Operations for Beneficial Ownership Data Standard (BODS) integration
//! and UBO discovery.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::helpers::extract_string_opt;
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use {crate::bods::UboDiscoveryService, sqlx::PgPool, std::sync::Arc, uuid::Uuid};

/// Discover UBOs for an entity using GLEIF + BODS data
///
/// Rationale: Requires querying GLEIF for reporting exceptions and BODS for person statements.
#[register_custom_op]
pub struct BodsDiscoverUbosOp;

#[async_trait]
impl CustomOperation for BodsDiscoverUbosOp {
    fn domain(&self) -> &'static str {
        "bods"
    }
    fn verb(&self) -> &'static str {
        "discover-ubos"
    }
    fn rationale(&self) -> &'static str {
        "Requires querying GLEIF for reporting exceptions and BODS for person statements"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Get entity ID
        let entity_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing :entity-id argument"))?;

        let save = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "save")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let service = UboDiscoveryService::new(Arc::new(pool.clone()));

        let result = if save {
            service.discover_and_save(entity_id).await?
        } else {
            service.discover_ubos(entity_id).await?
        };

        let ubos: Vec<serde_json::Value> = result
            .ubos
            .iter()
            .map(|ubo| {
                serde_json::json!({
                    "name": ubo.name,
                    "nationalities": ubo.nationalities,
                    "country_of_residence": ubo.country_of_residence,
                    "birth_date": ubo.birth_date,
                    "ownership_percentage": ubo.ownership_percentage,
                    "is_direct": ubo.is_direct,
                    "control_types": ubo.control_types,
                    "ubo_type": ubo.ubo_type.as_str(),
                    "confidence": ubo.confidence,
                    "source": ubo.source,
                })
            })
            .collect();

        Ok(ExecutionResult::Record(serde_json::json!({
            "entity_id": entity_id,
            "entity_lei": result.entity_lei,
            "is_complete": result.is_complete,
            "ubo_count": ubos.len(),
            "ubos": ubos,
            "gaps": result.gaps,
            "sources_queried": result.sources_queried,
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "entity_id": uuid::Uuid::new_v4(),
            "is_complete": false,
            "ubos": [],
            "gaps": ["No database connection"],
        })))
    }
}

/// Import BODS statements from data dump
///
/// Rationale: Bulk import requires parsing BODS JSON and inserting into multiple tables.
#[register_custom_op]
pub struct BodsImportOp;

#[async_trait]
impl CustomOperation for BodsImportOp {
    fn domain(&self) -> &'static str {
        "bods"
    }
    fn verb(&self) -> &'static str {
        "import"
    }
    fn rationale(&self) -> &'static str {
        "Bulk import requires parsing BODS JSON and inserting into multiple tables"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let source = extract_string_opt(verb_call, "source")
            .ok_or_else(|| anyhow::anyhow!(":source required"))?;

        let _file_path = extract_string_opt(verb_call, "file-path");

        // TODO: Implement BODS bulk import
        // This would parse the BODS JSON format and insert into:
        // - bods_entity_statements
        // - bods_person_statements
        // - bods_ownership_statements

        match source.as_str() {
            "FILE" => Err(anyhow::anyhow!(
                "BODS file import not yet implemented. Requires :file-path argument."
            )),
            "API" => Err(anyhow::anyhow!(
                "BODS API import not yet implemented. Requires OpenOwnership API integration."
            )),
            "OPENOWNERSHIP" => Err(anyhow::anyhow!("OpenOwnership import not yet implemented.")),
            _ => Err(anyhow::anyhow!(
                "Unknown source: {}. Use FILE, API, or OPENOWNERSHIP.",
                source
            )),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("BODS import requires database connection"))
    }
}

/// Get BODS statement by ID
///
/// Rationale: Query BODS tables for specific statement.
#[register_custom_op]
pub struct BodsGetStatementOp;

#[async_trait]
impl CustomOperation for BodsGetStatementOp {
    fn domain(&self) -> &'static str {
        "bods"
    }
    fn verb(&self) -> &'static str {
        "get-statement"
    }
    fn rationale(&self) -> &'static str {
        "Query BODS tables for specific statement"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let statement_id = extract_string_opt(verb_call, "statement-id")
            .ok_or_else(|| anyhow::anyhow!(":statement-id required"))?;

        let statement_type = extract_string_opt(verb_call, "statement-type");

        // Try to find in each table based on type or all
        let result = match statement_type.as_deref() {
            Some("entity") => {
                let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
                    r#"SELECT statement_id, legal_name, lei
                       FROM "ob-poc".bods_entity_statements
                       WHERE statement_id = $1"#,
                )
                .bind(&statement_id)
                .fetch_optional(pool)
                .await?;

                row.map(|(id, name, lei)| {
                    serde_json::json!({
                        "statement_type": "entity",
                        "statement_id": id,
                        "legal_name": name,
                        "lei": lei,
                    })
                })
            }
            Some("person") => {
                let row: Option<(String, Option<String>, Option<Vec<String>>)> = sqlx::query_as(
                    r#"SELECT statement_id, full_name, nationalities
                       FROM "ob-poc".bods_person_statements
                       WHERE statement_id = $1"#,
                )
                .bind(&statement_id)
                .fetch_optional(pool)
                .await?;

                row.map(|(id, name, nationalities)| {
                    serde_json::json!({
                        "statement_type": "person",
                        "statement_id": id,
                        "full_name": name,
                        "nationalities": nationalities,
                    })
                })
            }
            Some("ownership") => {
                let row: Option<(String, String, String, Option<rust_decimal::Decimal>)> =
                    sqlx::query_as(
                        r#"SELECT statement_id, subject_entity_statement_id,
                              interested_party_type, share_exact
                           FROM "ob-poc".bods_ownership_statements
                           WHERE statement_id = $1"#,
                    )
                    .bind(&statement_id)
                    .fetch_optional(pool)
                    .await?;

                row.map(|(id, subject, party_type, share)| {
                    serde_json::json!({
                        "statement_type": "ownership",
                        "statement_id": id,
                        "subject_entity_statement_id": subject,
                        "interested_party_type": party_type,
                        "share_exact": share,
                    })
                })
            }
            _ => {
                // Try all tables - first check entity statements
                let entity_row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
                    r#"SELECT statement_id, legal_name, lei
                       FROM "ob-poc".bods_entity_statements
                       WHERE statement_id = $1"#,
                )
                .bind(&statement_id)
                .fetch_optional(pool)
                .await?;

                if let Some((id, name, lei)) = entity_row {
                    Some(serde_json::json!({
                        "statement_type": "entity",
                        "statement_id": id,
                        "legal_name": name,
                        "lei": lei,
                    }))
                } else {
                    // Try person statements
                    let person_row: Option<(String, Option<String>, Option<Vec<String>>)> =
                        sqlx::query_as(
                            r#"SELECT statement_id, full_name, nationalities
                               FROM "ob-poc".bods_person_statements
                               WHERE statement_id = $1"#,
                        )
                        .bind(&statement_id)
                        .fetch_optional(pool)
                        .await?;

                    if let Some((id, name, nationalities)) = person_row {
                        Some(serde_json::json!({
                            "statement_type": "person",
                            "statement_id": id,
                            "full_name": name,
                            "nationalities": nationalities,
                        }))
                    } else {
                        // Try ownership statements
                        let ownership_row: Option<(
                            String,
                            String,
                            String,
                            Option<rust_decimal::Decimal>,
                        )> = sqlx::query_as(
                            r#"SELECT statement_id, subject_entity_statement_id,
                                  interested_party_type, share_exact
                               FROM "ob-poc".bods_ownership_statements
                               WHERE statement_id = $1"#,
                        )
                        .bind(&statement_id)
                        .fetch_optional(pool)
                        .await?;

                        ownership_row.map(|(id, subject, party_type, share)| {
                            serde_json::json!({
                                "statement_type": "ownership",
                                "statement_id": id,
                                "subject_entity_statement_id": subject,
                                "interested_party_type": party_type,
                                "share_exact": share,
                            })
                        })
                    }
                }
            }
        };

        match result {
            Some(r) => Ok(ExecutionResult::Record(r)),
            None => Err(anyhow::anyhow!("Statement not found: {}", statement_id)),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "BODS statement lookup requires database connection"
        ))
    }
}

/// Find BODS entity statement by LEI
///
/// Rationale: Query BODS entity statements table by LEI.
#[register_custom_op]
pub struct BodsFindByLeiOp;

#[async_trait]
impl CustomOperation for BodsFindByLeiOp {
    fn domain(&self) -> &'static str {
        "bods"
    }
    fn verb(&self) -> &'static str {
        "find-by-lei"
    }
    fn rationale(&self) -> &'static str {
        "Query BODS entity statements table by LEI"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let lei =
            extract_string_opt(verb_call, "lei").ok_or_else(|| anyhow::anyhow!(":lei required"))?;

        let row: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            r#"SELECT statement_id, legal_name, jurisdiction, entity_type
               FROM "ob-poc".bods_entity_statements
               WHERE lei = $1"#,
        )
        .bind(&lei)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((id, name, jurisdiction, entity_type)) => {
                Ok(ExecutionResult::Record(serde_json::json!({
                    "found": true,
                    "statement_id": id,
                    "lei": lei,
                    "legal_name": name,
                    "jurisdiction": jurisdiction,
                    "entity_type": entity_type,
                })))
            }
            None => Ok(ExecutionResult::Record(serde_json::json!({
                "found": false,
                "lei": lei,
                "message": "No BODS entity statement found for LEI"
            }))),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "found": false,
        })))
    }
}

/// List ownership statements for an entity
///
/// Rationale: Query BODS ownership statements by entity.
#[register_custom_op]
pub struct BodsListOwnershipOp;

#[async_trait]
impl CustomOperation for BodsListOwnershipOp {
    fn domain(&self) -> &'static str {
        "bods"
    }
    fn verb(&self) -> &'static str {
        "list-ownership"
    }
    fn rationale(&self) -> &'static str {
        "Query BODS ownership statements by entity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let subject_lei = extract_string_opt(verb_call, "subject-lei");
        let entity_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let include_expired = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "include-expired")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

        // Get LEI from entity-id if needed
        let lei = match (subject_lei, entity_id) {
            (Some(l), _) => l,
            (None, Some(eid)) => {
                let lei: Option<String> = sqlx::query_scalar(
                    r#"SELECT lei FROM "ob-poc".entity_limited_companies WHERE entity_id = $1"#,
                )
                .bind(eid)
                .fetch_optional(pool)
                .await?
                .flatten();

                lei.ok_or_else(|| anyhow::anyhow!("Entity {} has no LEI", eid))?
            }
            (None, None) => {
                return Err(anyhow::anyhow!(
                    "Either :subject-lei or :entity-id required"
                ));
            }
        };

        // Find BODS entity statement by LEI
        let entity_stmt: Option<String> = sqlx::query_scalar(
            r#"SELECT statement_id FROM "ob-poc".bods_entity_statements WHERE lei = $1"#,
        )
        .bind(&lei)
        .fetch_optional(pool)
        .await?;

        let entity_stmt_id = match entity_stmt {
            Some(id) => id,
            None => {
                return Ok(ExecutionResult::RecordSet(vec![]));
            }
        };

        // Query ownership statements
        let query = if include_expired {
            r#"SELECT o.statement_id, o.interested_party_type,
                      o.interested_party_statement_id, o.share_exact,
                      o.share_min, o.share_max, o.start_date, o.end_date,
                      p.full_name as person_name
               FROM "ob-poc".bods_ownership_statements o
               LEFT JOIN "ob-poc".bods_person_statements p
                 ON o.interested_party_statement_id = p.statement_id
               WHERE o.subject_entity_statement_id = $1"#
        } else {
            r#"SELECT o.statement_id, o.interested_party_type,
                      o.interested_party_statement_id, o.share_exact,
                      o.share_min, o.share_max, o.start_date, o.end_date,
                      p.full_name as person_name
               FROM "ob-poc".bods_ownership_statements o
               LEFT JOIN "ob-poc".bods_person_statements p
                 ON o.interested_party_statement_id = p.statement_id
               WHERE o.subject_entity_statement_id = $1
                 AND (o.end_date IS NULL OR o.end_date > CURRENT_DATE)"#
        };

        #[derive(sqlx::FromRow)]
        struct OwnershipRow {
            statement_id: String,
            interested_party_type: String,
            interested_party_statement_id: Option<String>,
            share_exact: Option<rust_decimal::Decimal>,
            share_min: Option<rust_decimal::Decimal>,
            share_max: Option<rust_decimal::Decimal>,
            start_date: Option<chrono::NaiveDate>,
            end_date: Option<chrono::NaiveDate>,
            person_name: Option<String>,
        }

        let rows: Vec<OwnershipRow> = sqlx::query_as(query)
            .bind(&entity_stmt_id)
            .fetch_all(pool)
            .await?;

        let results: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "statement_id": r.statement_id,
                    "interested_party_type": r.interested_party_type,
                    "interested_party_statement_id": r.interested_party_statement_id,
                    "person_name": r.person_name,
                    "share_exact": r.share_exact,
                    "share_min": r.share_min,
                    "share_max": r.share_max,
                    "start_date": r.start_date,
                    "end_date": r.end_date,
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(results))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

/// Sync BODS data from GLEIF reporting exceptions
///
/// Rationale: Queries GLEIF exceptions and populates BODS tables.
#[register_custom_op]
pub struct BodsSyncFromGleifOp;

#[async_trait]
impl CustomOperation for BodsSyncFromGleifOp {
    fn domain(&self) -> &'static str {
        "bods"
    }
    fn verb(&self) -> &'static str {
        "sync-from-gleif"
    }
    fn rationale(&self) -> &'static str {
        "Queries GLEIF exceptions and populates BODS tables"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "entity-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            });

        let limit = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "limit")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(100) as i32;

        let service = UboDiscoveryService::new(Arc::new(pool.clone()));

        match entity_id {
            Some(eid) => {
                // Sync single entity
                let result = service.discover_and_save(eid).await?;
                Ok(ExecutionResult::Record(serde_json::json!({
                    "synced": 1,
                    "entity_id": eid,
                    "ubos_found": result.ubos.len(),
                    "is_complete": result.is_complete,
                })))
            }
            None => {
                // Find entities with GLEIF exceptions that need BODS sync
                let entities: Vec<Uuid> = sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_limited_companies
                       WHERE (gleif_direct_parent_exception = 'NATURAL_PERSONS'
                              OR gleif_ultimate_parent_exception = 'NATURAL_PERSONS')
                         AND entity_id NOT IN (
                             SELECT DISTINCT entity_id FROM "ob-poc".entity_ubos
                         )
                       LIMIT $1"#,
                )
                .bind(limit)
                .fetch_all(pool)
                .await?;

                let mut synced = 0;
                let mut errors = 0;

                for eid in entities {
                    match service.discover_and_save(eid).await {
                        Ok(_) => synced += 1,
                        Err(e) => {
                            tracing::warn!("Failed to sync BODS for entity {}: {}", eid, e);
                            errors += 1;
                        }
                    }
                }

                Ok(ExecutionResult::Record(serde_json::json!({
                    "synced": synced,
                    "errors": errors,
                })))
            }
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "synced": 0,
            "errors": 0,
        })))
    }
}

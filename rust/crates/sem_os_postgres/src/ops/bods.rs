//! BODS (Beneficial Ownership Data Standard) verbs (6 plugin
//! verbs) — SemOS-side YAML-first re-implementation of the
//! plugin subset of `rust/config/verbs/bods.yaml`.
//!
//! - `discover-ubos` / `sync-from-gleif` — delegate to
//!   `dsl_runtime::bods::UboDiscoveryService` (transitional
//!   `Arc<PgPool>` clone; service still takes a pool).
//! - `get-statement` / `find-by-lei` / `list-ownership` —
//!   direct sqlx against the 3 BODS statement tables.
//! - `import` — `FILE`/`API`/`OPENOWNERSHIP` stubs (unimplemented,
//!   returns typed errors).

use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::bods::UboDiscoveryService;
use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string, json_extract_string_opt,
    json_extract_uuid, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── bods.discover-ubos ────────────────────────────────────────────────────────

pub struct DiscoverUbos;

#[async_trait]
impl SemOsVerbOp for DiscoverUbos {
    fn fqn(&self) -> &str {
        "bods.discover-ubos"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let save = json_extract_bool_opt(args, "save").unwrap_or(true);

        let service = UboDiscoveryService::new(Arc::new(scope.pool().clone()));
        let result = if save {
            service.discover_and_save(entity_id).await?
        } else {
            service.discover_ubos(entity_id).await?
        };

        let ubos: Vec<Value> = result
            .ubos
            .iter()
            .map(|ubo| {
                json!({
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

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id,
            "entity_lei": result.entity_lei,
            "is_complete": result.is_complete,
            "ubo_count": ubos.len(),
            "ubos": ubos,
            "gaps": result.gaps,
            "sources_queried": result.sources_queried,
        })))
    }
}

// ── bods.import ───────────────────────────────────────────────────────────────

pub struct Import;

#[async_trait]
impl SemOsVerbOp for Import {
    fn fqn(&self) -> &str {
        "bods.import"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let source = json_extract_string(args, "source")?;
        let _file_path = json_extract_string_opt(args, "file-path");

        match source.as_str() {
            "FILE" => Err(anyhow!(
                "BODS file import not yet implemented. Requires :file-path argument."
            )),
            "API" => Err(anyhow!(
                "BODS API import not yet implemented. Requires OpenOwnership API integration."
            )),
            "OPENOWNERSHIP" => Err(anyhow!("OpenOwnership import not yet implemented.")),
            _ => Err(anyhow!(
                "Unknown source: {}. Use FILE, API, or OPENOWNERSHIP.",
                source
            )),
        }
    }
}

// ── bods.get-statement ────────────────────────────────────────────────────────

pub struct GetStatement;

#[async_trait]
impl SemOsVerbOp for GetStatement {
    fn fqn(&self) -> &str {
        "bods.get-statement"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let statement_id = json_extract_string(args, "statement-id")?;
        let statement_type = json_extract_string_opt(args, "statement-type");

        async fn try_entity(
            scope: &mut dyn TransactionScope,
            id: &str,
        ) -> Result<Option<Value>> {
            let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
                r#"SELECT statement_id, legal_name, lei
                   FROM "ob-poc".bods_entity_statements
                   WHERE statement_id = $1"#,
            )
            .bind(id)
            .fetch_optional(scope.executor())
            .await?;
            Ok(row.map(|(id, name, lei)| {
                json!({
                    "statement_type": "entity",
                    "statement_id": id,
                    "legal_name": name,
                    "lei": lei,
                })
            }))
        }

        async fn try_person(
            scope: &mut dyn TransactionScope,
            id: &str,
        ) -> Result<Option<Value>> {
            let row: Option<(String, Option<String>, Option<Vec<String>>)> = sqlx::query_as(
                r#"SELECT statement_id, full_name, nationalities
                   FROM "ob-poc".bods_person_statements
                   WHERE statement_id = $1"#,
            )
            .bind(id)
            .fetch_optional(scope.executor())
            .await?;
            Ok(row.map(|(id, name, nationalities)| {
                json!({
                    "statement_type": "person",
                    "statement_id": id,
                    "full_name": name,
                    "nationalities": nationalities,
                })
            }))
        }

        async fn try_ownership(
            scope: &mut dyn TransactionScope,
            id: &str,
        ) -> Result<Option<Value>> {
            let row: Option<(String, String, String, Option<rust_decimal::Decimal>)> =
                sqlx::query_as(
                    r#"SELECT statement_id, subject_entity_statement_id,
                              interested_party_type, share_exact
                       FROM "ob-poc".bods_ownership_statements
                       WHERE statement_id = $1"#,
                )
                .bind(id)
                .fetch_optional(scope.executor())
                .await?;
            Ok(row.map(|(id, subject, party_type, share)| {
                json!({
                    "statement_type": "ownership",
                    "statement_id": id,
                    "subject_entity_statement_id": subject,
                    "interested_party_type": party_type,
                    "share_exact": share,
                })
            }))
        }

        let result = match statement_type.as_deref() {
            Some("entity") => try_entity(scope, &statement_id).await?,
            Some("person") => try_person(scope, &statement_id).await?,
            Some("ownership") => try_ownership(scope, &statement_id).await?,
            _ => {
                // Probe in order: entity → person → ownership
                if let Some(r) = try_entity(scope, &statement_id).await? {
                    Some(r)
                } else if let Some(r) = try_person(scope, &statement_id).await? {
                    Some(r)
                } else {
                    try_ownership(scope, &statement_id).await?
                }
            }
        };

        result
            .map(VerbExecutionOutcome::Record)
            .ok_or_else(|| anyhow!("Statement not found: {}", statement_id))
    }
}

// ── bods.find-by-lei ──────────────────────────────────────────────────────────

pub struct FindByLei;

#[async_trait]
impl SemOsVerbOp for FindByLei {
    fn fqn(&self) -> &str {
        "bods.find-by-lei"
    }
    async fn execute(
        &self,
        args: &Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let lei = json_extract_string(args, "lei")?;
        let row: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            r#"SELECT statement_id, legal_name, jurisdiction, entity_type
               FROM "ob-poc".bods_entity_statements
               WHERE lei = $1"#,
        )
        .bind(&lei)
        .fetch_optional(scope.executor())
        .await?;

        let result = match row {
            Some((id, name, jurisdiction, entity_type)) => json!({
                "found": true,
                "statement_id": id,
                "lei": lei,
                "legal_name": name,
                "jurisdiction": jurisdiction,
                "entity_type": entity_type,
            }),
            None => json!({
                "found": false,
                "lei": lei,
                "message": "No BODS entity statement found for LEI",
            }),
        };
        Ok(VerbExecutionOutcome::Record(result))
    }
}

// ── bods.list-ownership ───────────────────────────────────────────────────────

pub struct ListOwnership;

#[async_trait]
impl SemOsVerbOp for ListOwnership {
    fn fqn(&self) -> &str {
        "bods.list-ownership"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let subject_lei = json_extract_string_opt(args, "subject-lei");
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let include_expired = json_extract_bool_opt(args, "include-expired").unwrap_or(false);

        let lei = match (subject_lei, entity_id) {
            (Some(l), _) => l,
            (None, Some(eid)) => {
                let lei: Option<Option<String>> = sqlx::query_scalar(
                    r#"SELECT lei FROM "ob-poc".entity_limited_companies WHERE entity_id = $1"#,
                )
                .bind(eid)
                .fetch_optional(scope.executor())
                .await?;
                lei.flatten()
                    .ok_or_else(|| anyhow!("Entity {} has no LEI", eid))?
            }
            (None, None) => return Err(anyhow!("Either :subject-lei or :entity-id required")),
        };

        let entity_stmt: Option<String> = sqlx::query_scalar(
            r#"SELECT statement_id FROM "ob-poc".bods_entity_statements WHERE lei = $1"#,
        )
        .bind(&lei)
        .fetch_optional(scope.executor())
        .await?;

        let entity_stmt_id = match entity_stmt {
            Some(id) => id,
            None => return Ok(VerbExecutionOutcome::RecordSet(vec![])),
        };

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
            .fetch_all(scope.executor())
            .await?;

        let results: Vec<Value> = rows
            .into_iter()
            .map(|r| {
                json!({
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

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// ── bods.sync-from-gleif ──────────────────────────────────────────────────────

pub struct SyncFromGleif;

#[async_trait]
impl SemOsVerbOp for SyncFromGleif {
    fn fqn(&self) -> &str {
        "bods.sync-from-gleif"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid_opt(args, ctx, "entity-id");
        let limit = json_extract_int_opt(args, "limit").unwrap_or(100) as i32;
        let service = UboDiscoveryService::new(Arc::new(scope.pool().clone()));

        match entity_id {
            Some(eid) => {
                let result = service.discover_and_save(eid).await?;
                Ok(VerbExecutionOutcome::Record(json!({
                    "synced": 1,
                    "entity_id": eid,
                    "ubos_found": result.ubos.len(),
                    "is_complete": result.is_complete,
                })))
            }
            None => {
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
                .fetch_all(scope.executor())
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

                Ok(VerbExecutionOutcome::Record(json!({
                    "synced": synced,
                    "errors": errors,
                })))
            }
        }
    }
}

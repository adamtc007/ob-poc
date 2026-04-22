//! Research source loader verbs (15 plugin verbs) — `research.sources.*`,
//! `research.companies-house.*`, `research.sec-edgar.*`. Plugin handlers
//! for the SourceLoader trait, enabling DSL verbs to interact with
//! pluggable research data sources (GLEIF, Companies House, SEC EDGAR, etc.).
//!
//! Phase 5c-migrate Phase B Pattern B slice #75: ported from
//! `CustomOperation` + `inventory::collect!` to `SemOsVerbOp`. Stays in
//! `ob-poc::domain_ops::source_loader_ops` because the ops bridge to
//! `crate::research::sources::*` — upstream of `sem_os_postgres`.
//!
//! Rationale: These operations require external API calls and the
//! SourceRegistry abstraction layer to search, fetch, and normalize
//! data from multiple sources.

use anyhow::Result;
use async_trait::async_trait;
use sem_os_postgres::ops::SemOsVerbOp;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_int_opt, json_extract_string_opt, json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

#[cfg(feature = "database")]
use {
    crate::research::sources::{
        normalized::{EntityType, NormalizedEntity},
        registry::SourceRegistry,
        traits::{
            FetchControlHoldersOptions, FetchOfficersOptions, FetchOptions, SearchOptions,
            SourceLoader,
        },
        CompaniesHouseLoader, GleifLoader, SecEdgarLoader,
    },
    sqlx::PgPool,
    std::sync::Arc,
};

// =============================================================================
// Generic Source Operations (research.sources domain)
// =============================================================================

/// List all available research sources
pub struct SourcesList;

#[async_trait]
impl SemOsVerbOp for SourcesList {
    fn fqn(&self) -> &str {
        "research.sources.list"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let registry = build_source_registry();

        let sources: Vec<serde_json::Value> = registry
            .list()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "source_id": s.id,
                    "source_name": s.name,
                    "jurisdictions": s.jurisdictions,
                    "provides": s.provides.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                    "key_type": s.key_type,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(sources))
    }
}

/// Get information about a specific research source
pub struct SourcesInfo;

#[async_trait]
impl SemOsVerbOp for SourcesInfo {
    fn fqn(&self) -> &str {
        "research.sources.info"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let source_id = json_extract_string_opt(args, "source-id")
            .ok_or_else(|| anyhow::anyhow!(":source-id required"))?;

        let registry = build_source_registry();

        let source = registry
            .get(&source_id)
            .ok_or_else(|| anyhow::anyhow!("Source not found: {}", source_id))?;

        let result = serde_json::json!({
            "source_id": source.source_id(),
            "source_name": source.source_name(),
            "jurisdictions": source.jurisdictions(),
            "provides": source.provides().iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            "key_type": source.key_type(),
        });

        Ok(VerbExecutionOutcome::Record(result))
    }
}

/// Search a specific source for entities
pub struct SourcesSearch;

#[async_trait]
impl SemOsVerbOp for SourcesSearch {
    fn fqn(&self) -> &str {
        "research.sources.search"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let source_id = json_extract_string_opt(args, "source-id")
            .ok_or_else(|| anyhow::anyhow!(":source-id required"))?;
        let query = json_extract_string_opt(args, "query")
            .ok_or_else(|| anyhow::anyhow!(":query required"))?;
        let jurisdiction = json_extract_string_opt(args, "jurisdiction");
        let limit = json_extract_int_opt(args, "limit").map(|l| l as usize);
        let include_inactive = json_extract_bool_opt(args, "include-inactive").unwrap_or(false);

        let registry = build_source_registry();

        let source = registry
            .get(&source_id)
            .ok_or_else(|| anyhow::anyhow!("Source not found: {}", source_id))?;

        let mut options = SearchOptions::new();
        if let Some(j) = jurisdiction {
            options = options.with_jurisdiction(j);
        }
        if let Some(l) = limit {
            options = options.with_limit(l);
        }
        if include_inactive {
            options = options.include_inactive();
        }

        let candidates = source.search(&query, Some(options)).await?;

        let results: Vec<serde_json::Value> = candidates
            .iter()
            .map(|c| {
                serde_json::json!({
                    "source_key": c.key,
                    "name": c.name,
                    "jurisdiction": c.jurisdiction,
                    "status": c.status,
                    "score": c.score,
                    "metadata": c.metadata,
                })
            })
            .collect();

        Ok(Some(serde_json::json!({ "_sources_search_results": results })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let results = args
            .get("_sources_search_results")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "sources.search: pre_fetch result missing \
                     (`_sources_search_results` absent from args)"
                )
            })?;
        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

/// Fetch entity data from a source by key
pub struct SourcesFetch;

#[async_trait]
impl SemOsVerbOp for SourcesFetch {
    fn fqn(&self) -> &str {
        "research.sources.fetch"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let source_id = json_extract_string_opt(args, "source-id")
            .ok_or_else(|| anyhow::anyhow!(":source-id required"))?;
        let key = json_extract_string_opt(args, "key")
            .ok_or_else(|| anyhow::anyhow!(":key required"))?;
        let include_raw = json_extract_bool_opt(args, "include-raw").unwrap_or(false);
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");

        let registry = build_source_registry();

        let source = registry
            .get(&source_id)
            .ok_or_else(|| anyhow::anyhow!("Source not found: {}", source_id))?;

        if !source.validate_key(&key) {
            return Err(anyhow::anyhow!(
                "Invalid key format for {}: {}",
                source_id,
                key
            ));
        }

        let mut options = FetchOptions::new();
        if include_raw {
            options = options.with_raw();
        }
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }

        let entity = source.fetch_entity(&key, Some(options)).await?;
        let result = normalized_entity_to_json(&entity);

        Ok(Some(serde_json::json!({ "_sources_fetched_entity": result })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let result = args.get("_sources_fetched_entity").cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "sources.fetch: pre_fetch result missing \
                 (`_sources_fetched_entity` absent from args)"
            )
        })?;

        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");
        if let Some(dec_id) = decision_id {
            // `source_id` is needed for the log tag; re-read from args.
            let source_id = json_extract_string_opt(args, "source-id")
                .ok_or_else(|| anyhow::anyhow!(":source-id required"))?;
            let pool = scope.pool().clone();
            log_research_action(&pool, dec_id, &format!("{}:fetch", source_id), &result, 0, 0)
                .await?;
        }

        Ok(VerbExecutionOutcome::Record(result))
    }
}

/// Find the best source for a jurisdiction and data type
pub struct SourcesFindForJurisdiction;

#[async_trait]
impl SemOsVerbOp for SourcesFindForJurisdiction {
    fn fqn(&self) -> &str {
        "research.sources.find-for-jurisdiction"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let jurisdiction = json_extract_string_opt(args, "jurisdiction")
            .ok_or_else(|| anyhow::anyhow!(":jurisdiction required"))?;
        let data_type = json_extract_string_opt(args, "data-type")
            .ok_or_else(|| anyhow::anyhow!(":data-type required"))?;

        let registry = build_source_registry();

        // Parse data type string to SourceDataType enum
        let data_type_enum = match data_type.as_str() {
            "entity" => crate::research::sources::traits::SourceDataType::Entity,
            "control-holders" => crate::research::sources::traits::SourceDataType::ControlHolders,
            "officers" => crate::research::sources::traits::SourceDataType::Officers,
            "parent-chain" => crate::research::sources::traits::SourceDataType::ParentChain,
            "subsidiaries" => crate::research::sources::traits::SourceDataType::Subsidiaries,
            "filings" => crate::research::sources::traits::SourceDataType::Filings,
            _ => return Err(anyhow::anyhow!("Unknown data type: {}", data_type)),
        };

        let sources = registry.find_for_jurisdiction(&jurisdiction, data_type_enum);

        let results: Vec<serde_json::Value> = sources
            .iter()
            .map(|s| {
                serde_json::json!({
                    "source_id": s.source_id(),
                    "source_name": s.source_name(),
                    "key_type": s.key_type(),
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// =============================================================================
// Companies House Operations (research.companies-house domain)
// =============================================================================

/// Search Companies House for UK companies
pub struct CompaniesHouseSearch;

#[async_trait]
impl SemOsVerbOp for CompaniesHouseSearch {
    fn fqn(&self) -> &str {
        "research.companies-house.search"
    }

    /// Phase F.2 (ledger §3.2, 2026-04-22): HTTP call moved to pre_fetch.
    /// Executes outside the transaction scope; result is serialized into
    /// args under `_search_results` for `execute` to hand back.
    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let query = json_extract_string_opt(args, "query")
            .ok_or_else(|| anyhow::anyhow!(":query required"))?;
        let limit = json_extract_int_opt(args, "limit").map(|l| l as usize);
        let include_inactive = json_extract_bool_opt(args, "include-inactive").unwrap_or(false);

        let loader = CompaniesHouseLoader::from_env()?;

        let mut options = SearchOptions::new().with_jurisdiction("GB");
        if let Some(l) = limit {
            options = options.with_limit(l);
        }
        if include_inactive {
            options = options.include_inactive();
        }

        let candidates = loader.search(&query, Some(options)).await?;

        let results: Vec<serde_json::Value> = candidates
            .iter()
            .map(|c| {
                serde_json::json!({
                    "company_number": c.key,
                    "name": c.name,
                    "status": c.status,
                    "score": c.score,
                    "metadata": c.metadata,
                })
            })
            .collect();

        Ok(Some(serde_json::json!({ "_search_results": results })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let results = args
            .get("_search_results")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "companies-house.search: pre_fetch result missing \
                     (`_search_results` absent from args)"
                )
            })?;
        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

/// Fetch company profile from Companies House
pub struct CompaniesHouseFetchCompany;

#[async_trait]
impl SemOsVerbOp for CompaniesHouseFetchCompany {
    fn fqn(&self) -> &str {
        "research.companies-house.fetch-company"
    }

    /// Phase F.2 (2026-04-22): HTTP fetch moves to pre_fetch; the DB log
    /// (log_research_action) stays in execute so it shares the inner
    /// transaction scope.
    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let company_number = json_extract_string_opt(args, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");

        let loader = CompaniesHouseLoader::from_env()?;

        let mut options = FetchOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }

        let entity = loader.fetch_entity(&company_number, Some(options)).await?;
        let result = normalized_entity_to_json(&entity);

        Ok(Some(serde_json::json!({ "_fetched_entity": result })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let result = args.get("_fetched_entity").cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "companies-house.fetch-company: pre_fetch result missing \
                 (`_fetched_entity` absent from args)"
            )
        })?;

        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");
        if let Some(dec_id) = decision_id {
            let pool = scope.pool().clone();
            log_research_action(&pool, dec_id, "companies-house:fetch-company", &result, 0, 0)
                .await?;
        }

        Ok(VerbExecutionOutcome::Record(result))
    }
}

/// Fetch PSC (Persons with Significant Control) from Companies House
pub struct CompaniesHouseFetchPsc;

#[async_trait]
impl SemOsVerbOp for CompaniesHouseFetchPsc {
    fn fqn(&self) -> &str {
        "research.companies-house.fetch-psc"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let company_number = json_extract_string_opt(args, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let include_ceased = json_extract_bool_opt(args, "include-ceased").unwrap_or(false);
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");

        let loader = CompaniesHouseLoader::from_env()?;

        let mut options = FetchControlHoldersOptions::new();
        if include_ceased {
            options = options.include_ceased();
        }
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }

        let holders = loader
            .fetch_control_holders(&company_number, Some(options))
            .await?;

        let results: Vec<serde_json::Value> = holders
            .iter()
            .map(|h| {
                serde_json::json!({
                    "name": h.holder_name,
                    "holder_type": h.holder_type.to_string(),
                    "nationality": h.nationality,
                    "country_of_residence": h.country_of_residence,
                    "ownership_pct_low": h.ownership_pct_low,
                    "ownership_pct_high": h.ownership_pct_high,
                    "ownership_pct_exact": h.ownership_pct_exact,
                    "notified_date": h.notified_on,
                    "ceased_date": h.ceased_on,
                    "natures_of_control": h.natures_of_control,
                })
            })
            .collect();

        Ok(Some(serde_json::json!({ "_psc_holders": results })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let results = args
            .get("_psc_holders")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "companies-house.fetch-psc: pre_fetch result missing \
                     (`_psc_holders` absent from args)"
                )
            })?;

        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");
        if let Some(dec_id) = decision_id {
            let pool = scope.pool().clone();
            log_research_action(
                &pool,
                dec_id,
                "companies-house:fetch-psc",
                &serde_json::json!({ "holders": results.len() }),
                0,
                0,
            )
            .await?;
        }

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

/// Fetch officers (directors, secretaries) from Companies House
pub struct CompaniesHouseFetchOfficers;

#[async_trait]
impl SemOsVerbOp for CompaniesHouseFetchOfficers {
    fn fqn(&self) -> &str {
        "research.companies-house.fetch-officers"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let company_number = json_extract_string_opt(args, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let include_resigned = json_extract_bool_opt(args, "include-resigned").unwrap_or(false);

        let loader = CompaniesHouseLoader::from_env()?;

        let mut options = FetchOfficersOptions::new();
        if include_resigned {
            options = options.include_resigned();
        }

        let officers = loader
            .fetch_officers(&company_number, Some(options))
            .await?;

        let results: Vec<serde_json::Value> = officers
            .iter()
            .map(|o| {
                serde_json::json!({
                    "name": o.name,
                    "role": o.role.to_string(),
                    "nationality": o.nationality,
                    "country_of_residence": o.country_of_residence,
                    "appointed_date": o.appointed_date,
                    "resigned_date": o.resigned_date,
                    "occupation": o.occupation,
                })
            })
            .collect();

        Ok(Some(serde_json::json!({ "_officers": results })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let results = args
            .get("_officers")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "companies-house.fetch-officers: pre_fetch result missing \
                     (`_officers` absent from args)"
                )
            })?;
        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

/// Import company (and optionally PSC/officers) from Companies House to database
pub struct CompaniesHouseImportCompany;

#[async_trait]
impl SemOsVerbOp for CompaniesHouseImportCompany {
    fn fqn(&self) -> &str {
        "research.companies-house.import-company"
    }

    /// Phase F.2: ALL external HTTP (entity + optional PSC + optional
    /// officers) runs in pre_fetch. DB writes (create_entity,
    /// log_research_action) stay in execute where they share the inner
    /// transaction scope.
    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let company_number = json_extract_string_opt(args, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let include_psc = json_extract_bool_opt(args, "include-psc").unwrap_or(true);
        let include_officers = json_extract_bool_opt(args, "include-officers").unwrap_or(false);
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");

        let loader = CompaniesHouseLoader::from_env()?;

        let mut options = FetchOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }
        let entity = loader.fetch_entity(&company_number, Some(options)).await?;

        let mut psc_count = 0;
        if include_psc {
            let psc_options = FetchControlHoldersOptions::new();
            let holders = loader
                .fetch_control_holders(&company_number, Some(psc_options))
                .await?;
            psc_count = holders.len();
        }

        let mut officer_count = 0;
        if include_officers {
            let officer_options = FetchOfficersOptions::new();
            let officers = loader
                .fetch_officers(&company_number, Some(officer_options))
                .await?;
            officer_count = officers.len();
        }

        Ok(Some(serde_json::json!({
            "_ch_import_entity": serde_json::to_value(&entity)?,
            "_ch_import_psc_count": psc_count,
            "_ch_import_officer_count": officer_count,
        })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let company_number = json_extract_string_opt(args, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");
        let pool = scope.pool().clone();

        let entity: crate::research::sources::normalized::NormalizedEntity = args
            .get("_ch_import_entity")
            .cloned()
            .map(serde_json::from_value)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "companies-house.import-company: pre_fetch result missing \
                     (`_ch_import_entity` absent from args)"
                )
            })??;
        let psc_count = args
            .get("_ch_import_psc_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let officer_count = args
            .get("_ch_import_officer_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let entity_id = create_entity_from_normalized(&pool, &entity).await?;

        let result = serde_json::json!({
            "entity_id": entity_id,
            "company_number": company_number,
            "name": entity.name,
            "psc_imported": psc_count,
            "officers_imported": officer_count,
        });

        // Log research action if decision-id provided
        if let Some(dec_id) = decision_id {
            log_research_action(
                &pool,
                dec_id,
                "companies-house:import-company",
                &result,
                1,
                0,
            )
            .await?;
        }

        Ok(VerbExecutionOutcome::Record(result))
    }
}

// =============================================================================
// SEC EDGAR Operations (research.sec-edgar domain)
// =============================================================================

/// Search SEC EDGAR for US public companies
pub struct SecEdgarSearch;

#[async_trait]
impl SemOsVerbOp for SecEdgarSearch {
    fn fqn(&self) -> &str {
        "research.sec-edgar.search"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let query = json_extract_string_opt(args, "query")
            .ok_or_else(|| anyhow::anyhow!(":query required"))?;
        let limit = json_extract_int_opt(args, "limit").map(|l| l as usize);

        let loader = SecEdgarLoader::new()?;

        let mut options = SearchOptions::new().with_jurisdiction("US");
        if let Some(l) = limit {
            options = options.with_limit(l);
        }

        let candidates = loader.search(&query, Some(options)).await?;

        let results: Vec<serde_json::Value> = candidates
            .iter()
            .map(|c| {
                serde_json::json!({
                    "cik": c.key,
                    "name": c.name,
                    "score": c.score,
                    "metadata": c.metadata,
                })
            })
            .collect();

        Ok(Some(serde_json::json!({ "_sec_search_results": results })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let results = args
            .get("_sec_search_results")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "sec-edgar.search: pre_fetch result missing \
                     (`_sec_search_results` absent from args)"
                )
            })?;
        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

/// Fetch company from SEC EDGAR
pub struct SecEdgarFetchCompany;

#[async_trait]
impl SemOsVerbOp for SecEdgarFetchCompany {
    fn fqn(&self) -> &str {
        "research.sec-edgar.fetch-company"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let cik = json_extract_string_opt(args, "cik")
            .ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");

        let loader = SecEdgarLoader::new()?;

        let mut options = FetchOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }

        let entity = loader.fetch_entity(&cik, Some(options)).await?;
        let result = normalized_entity_to_json(&entity);

        Ok(Some(serde_json::json!({ "_sec_fetched_entity": result })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let result = args.get("_sec_fetched_entity").cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "sec-edgar.fetch-company: pre_fetch result missing \
                 (`_sec_fetched_entity` absent from args)"
            )
        })?;

        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");
        if let Some(dec_id) = decision_id {
            let pool = scope.pool().clone();
            log_research_action(&pool, dec_id, "sec-edgar:fetch-company", &result, 0, 0).await?;
        }

        Ok(VerbExecutionOutcome::Record(result))
    }
}

/// Fetch 13D/13G beneficial ownership filings from SEC EDGAR
pub struct SecEdgarFetchBeneficialOwners;

#[async_trait]
impl SemOsVerbOp for SecEdgarFetchBeneficialOwners {
    fn fqn(&self) -> &str {
        "research.sec-edgar.fetch-beneficial-owners"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let cik = json_extract_string_opt(args, "cik")
            .ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let _include_13d = json_extract_bool_opt(args, "include-13d").unwrap_or(true);
        let _include_13g = json_extract_bool_opt(args, "include-13g").unwrap_or(true);
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");

        let loader = SecEdgarLoader::new()?;

        let mut options = FetchControlHoldersOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }

        let holders = loader.fetch_control_holders(&cik, Some(options)).await?;

        let results: Vec<serde_json::Value> = holders
            .iter()
            .map(|h| {
                serde_json::json!({
                    "name": h.holder_name,
                    "holder_type": h.holder_type.to_string(),
                    "ownership_pct_low": h.ownership_pct_low,
                    "ownership_pct_high": h.ownership_pct_high,
                    "notified_date": h.notified_on,
                    "source_document": h.source_document,
                })
            })
            .collect();

        Ok(Some(serde_json::json!({ "_sec_beneficial_owners": results })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let results = args
            .get("_sec_beneficial_owners")
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "sec-edgar.fetch-beneficial-owners: pre_fetch result missing \
                     (`_sec_beneficial_owners` absent from args)"
                )
            })?;

        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");
        if let Some(dec_id) = decision_id {
            let pool = scope.pool().clone();
            log_research_action(
                &pool,
                dec_id,
                "sec-edgar:fetch-beneficial-owners",
                &serde_json::json!({ "holders": results.len() }),
                0,
                0,
            )
            .await?;
        }

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

/// Fetch recent SEC filings
pub struct SecEdgarFetchFilings;

#[async_trait]
impl SemOsVerbOp for SecEdgarFetchFilings {
    fn fqn(&self) -> &str {
        "research.sec-edgar.fetch-filings"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let cik = json_extract_string_opt(args, "cik")
            .ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let _limit = json_extract_int_opt(args, "limit").unwrap_or(50);

        let loader = SecEdgarLoader::new()?;

        let options = FetchOptions::new().with_raw();
        let entity = loader.fetch_entity(&cik, Some(options)).await?;

        // Extract filings from raw_response field
        let filings = entity
            .raw_response
            .as_ref()
            .and_then(|r| r.get("filings"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));

        Ok(Some(serde_json::json!({ "_sec_filings": filings })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let filings = args.get("_sec_filings").cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "sec-edgar.fetch-filings: pre_fetch result missing \
                 (`_sec_filings` absent from args)"
            )
        })?;
        Ok(VerbExecutionOutcome::Record(
            serde_json::json!({ "filings": filings }),
        ))
    }
}

/// Import SEC company to database
pub struct SecEdgarImportCompany;

#[async_trait]
impl SemOsVerbOp for SecEdgarImportCompany {
    fn fqn(&self) -> &str {
        "research.sec-edgar.import-company"
    }

    /// Phase F.2: HTTP fetches (entity + optional BO) in pre_fetch; DB
    /// writes (create_entity, log_research_action) in execute.
    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<Option<serde_json::Value>> {
        let cik = json_extract_string_opt(args, "cik")
            .ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let include_beneficial_owners =
            json_extract_bool_opt(args, "include-beneficial-owners").unwrap_or(true);
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");

        let loader = SecEdgarLoader::new()?;

        let mut options = FetchOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }
        let entity = loader.fetch_entity(&cik, Some(options)).await?;

        let mut bo_count = 0;
        if include_beneficial_owners {
            let bo_options = FetchControlHoldersOptions::new();
            let holders = loader.fetch_control_holders(&cik, Some(bo_options)).await?;
            bo_count = holders.len();
        }

        Ok(Some(serde_json::json!({
            "_sec_import_entity": serde_json::to_value(&entity)?,
            "_sec_import_bo_count": bo_count,
        })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cik = json_extract_string_opt(args, "cik")
            .ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let decision_id = json_extract_uuid_opt(args, ctx, "decision-id");
        let pool = scope.pool().clone();

        let entity: crate::research::sources::normalized::NormalizedEntity = args
            .get("_sec_import_entity")
            .cloned()
            .map(serde_json::from_value)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "sec-edgar.import-company: pre_fetch result missing \
                     (`_sec_import_entity` absent from args)"
                )
            })??;
        let bo_count = args
            .get("_sec_import_bo_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let entity_id = create_entity_from_normalized(&pool, &entity).await?;

        let result = serde_json::json!({
            "entity_id": entity_id,
            "cik": cik,
            "name": entity.name,
            "beneficial_owners_imported": bo_count,
        });

        if let Some(dec_id) = decision_id {
            log_research_action(&pool, dec_id, "sec-edgar:import-company", &result, 1, 0).await?;
        }

        Ok(VerbExecutionOutcome::Record(result))
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Build the source registry with all available loaders
#[cfg(feature = "database")]
fn build_source_registry() -> SourceRegistry {
    let mut registry = SourceRegistry::new();

    // Register GLEIF loader
    if let Ok(gleif) = GleifLoader::new() {
        registry.register(Arc::new(gleif));
    }

    // Register Companies House loader (requires API key)
    if let Ok(ch) = CompaniesHouseLoader::from_env() {
        registry.register(Arc::new(ch));
    }

    // Register SEC EDGAR loader
    if let Ok(sec) = SecEdgarLoader::new() {
        registry.register(Arc::new(sec));
    }

    registry
}

/// Convert NormalizedEntity to JSON
#[cfg(feature = "database")]
fn normalized_entity_to_json(entity: &NormalizedEntity) -> serde_json::Value {
    serde_json::json!({
        "source_key": entity.source_key,
        "source_name": entity.source_name,
        "name": entity.name,
        "lei": entity.lei,
        "registration_number": entity.registration_number,
        "entity_type": entity.entity_type.as_ref().map(|t| t.to_string()),
        "status": entity.status.as_ref().map(|s| s.to_string()),
        "jurisdiction": entity.jurisdiction,
        "incorporated_date": entity.incorporated_date,
        "dissolved_date": entity.dissolved_date,
        "registered_address": entity.registered_address.as_ref().map(|a| a.to_single_line()),
        "business_address": entity.business_address.as_ref().map(|a| a.to_single_line()),
    })
}

/// Create entity in database from normalized entity
#[cfg(feature = "database")]
async fn create_entity_from_normalized(
    pool: &PgPool,
    entity: &NormalizedEntity,
) -> Result<uuid::Uuid> {
    use uuid::Uuid;

    // Map entity type to type_code
    let type_code = match &entity.entity_type {
        Some(EntityType::LimitedCompany) => "limited_company",
        Some(EntityType::PublicCompany) => "limited_company", // Map to same type
        Some(EntityType::Partnership) => "partnership",
        Some(EntityType::Llp) => "llp",
        Some(EntityType::Fund) => "fund_standalone",
        Some(EntityType::Trust) => "trust",
        Some(EntityType::NaturalPerson) => "individual",
        Some(EntityType::Government) => "government",
        Some(EntityType::SoleProprietor) => "individual",
        Some(EntityType::Branch) => "limited_company",
        Some(EntityType::Unknown(_)) => "limited_company", // Default for unknown
        None => "limited_company",                         // Default when type is not specified
    };

    // Get entity type ID
    let entity_type_id: Uuid = sqlx::query_scalar(
        r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = $1"#,
    )
    .bind(type_code)
    .fetch_one(pool)
    .await?;

    // Create base entity
    let entity_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
           VALUES ($1, $2, $3)"#,
    )
    .bind(entity_id)
    .bind(entity_type_id)
    .bind(&entity.name)
    .execute(pool)
    .await?;

    // Create limited company record with source-specific identifiers
    if type_code == "limited_company" || type_code == "llp" {
        sqlx::query(
            r#"INSERT INTO "ob-poc".entity_limited_companies
               (entity_id, company_name, jurisdiction, lei, company_number, sec_cik)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(entity_id)
        .bind(&entity.name)
        .bind(&entity.jurisdiction)
        .bind(&entity.lei)
        .bind(&entity.registration_number)
        .bind::<Option<String>>(None) // SEC CIK could be extracted from source_key if SEC
        .execute(pool)
        .await?;
    }

    Ok(entity_id)
}

/// Log research action for audit trail
#[cfg(feature = "database")]
async fn log_research_action(
    pool: &PgPool,
    decision_id: uuid::Uuid,
    verb: &str,
    result: &serde_json::Value,
    entities_created: i32,
    entities_updated: i32,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "ob-poc".research_actions
           (decision_id, verb, result_summary, entities_created, entities_updated)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(decision_id)
    .bind(verb)
    .bind(result)
    .bind(entities_created)
    .bind(entities_updated)
    .execute(pool)
    .await?;

    Ok(())
}

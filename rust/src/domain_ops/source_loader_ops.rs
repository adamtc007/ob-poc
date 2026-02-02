//! Research Source Loader Custom Operations
//!
//! Plugin handlers for the SourceLoader trait, enabling DSL verbs to interact with
//! pluggable research data sources (GLEIF, Companies House, SEC EDGAR, etc.).
//!
//! Rationale: These operations require external API calls and the SourceRegistry
//! abstraction layer to search, fetch, and normalize data from multiple sources.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::helpers::{extract_bool_opt, extract_int_opt, extract_string_opt, extract_uuid_opt};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

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
#[register_custom_op]
pub struct SourceListOp;

#[async_trait]
impl CustomOperation for SourceListOp {
    fn domain(&self) -> &'static str {
        "research.sources"
    }
    fn verb(&self) -> &'static str {
        "list"
    }
    fn rationale(&self) -> &'static str {
        "Lists all available research sources from the SourceRegistry"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
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

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::json!(sources));
        }

        Ok(ExecutionResult::RecordSet(sources))
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

/// Get information about a specific research source
#[register_custom_op]
pub struct SourceInfoOp;

#[async_trait]
impl CustomOperation for SourceInfoOp {
    fn domain(&self) -> &'static str {
        "research.sources"
    }
    fn verb(&self) -> &'static str {
        "info"
    }
    fn rationale(&self) -> &'static str {
        "Returns detailed information about a specific research source"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let source_id = extract_string_opt(verb_call, "source-id")
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

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, result.clone());
        }

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({})))
    }
}

/// Search a specific source for entities
#[register_custom_op]
pub struct SourceSearchOp;

#[async_trait]
impl CustomOperation for SourceSearchOp {
    fn domain(&self) -> &'static str {
        "research.sources"
    }
    fn verb(&self) -> &'static str {
        "search"
    }
    fn rationale(&self) -> &'static str {
        "Searches a specific source for entities by name - requires external API call"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let source_id = extract_string_opt(verb_call, "source-id")
            .ok_or_else(|| anyhow::anyhow!(":source-id required"))?;
        let query = extract_string_opt(verb_call, "query")
            .ok_or_else(|| anyhow::anyhow!(":query required"))?;
        let jurisdiction = extract_string_opt(verb_call, "jurisdiction");
        let limit = extract_int_opt(verb_call, "limit").map(|l| l as usize);
        let include_inactive = extract_bool_opt(verb_call, "include-inactive").unwrap_or(false);

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

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::json!(results));
        }

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

/// Fetch entity data from a source by key
#[register_custom_op]
pub struct SourceFetchOp;

#[async_trait]
impl CustomOperation for SourceFetchOp {
    fn domain(&self) -> &'static str {
        "research.sources"
    }
    fn verb(&self) -> &'static str {
        "fetch"
    }
    fn rationale(&self) -> &'static str {
        "Fetches entity data from a source by key - requires external API call"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let source_id = extract_string_opt(verb_call, "source-id")
            .ok_or_else(|| anyhow::anyhow!(":source-id required"))?;
        let key =
            extract_string_opt(verb_call, "key").ok_or_else(|| anyhow::anyhow!(":key required"))?;
        let include_raw = extract_bool_opt(verb_call, "include-raw").unwrap_or(false);
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

        let registry = build_source_registry();

        let source = registry
            .get(&source_id)
            .ok_or_else(|| anyhow::anyhow!("Source not found: {}", source_id))?;

        // Validate key format
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

        // Log research action if decision-id provided
        if let Some(dec_id) = decision_id {
            log_research_action(pool, dec_id, &format!("{}:fetch", source_id), &result, 0, 0)
                .await?;
        }

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, result.clone());
        }

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({})))
    }
}

/// Find the best source for a jurisdiction and data type
#[register_custom_op]
pub struct SourceFindForJurisdictionOp;

#[async_trait]
impl CustomOperation for SourceFindForJurisdictionOp {
    fn domain(&self) -> &'static str {
        "research.sources"
    }
    fn verb(&self) -> &'static str {
        "find-for-jurisdiction"
    }
    fn rationale(&self) -> &'static str {
        "Finds the best available source for a given jurisdiction and data type"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let jurisdiction = extract_string_opt(verb_call, "jurisdiction")
            .ok_or_else(|| anyhow::anyhow!(":jurisdiction required"))?;
        let data_type = extract_string_opt(verb_call, "data-type")
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

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::json!(results));
        }

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

// =============================================================================
// Companies House Operations (research.companies-house domain)
// =============================================================================

/// Search Companies House for UK companies
#[register_custom_op]
pub struct ChSearchOp;

#[async_trait]
impl CustomOperation for ChSearchOp {
    fn domain(&self) -> &'static str {
        "research.companies-house"
    }
    fn verb(&self) -> &'static str {
        "search"
    }
    fn rationale(&self) -> &'static str {
        "Searches UK Companies House API for companies by name"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let query = extract_string_opt(verb_call, "query")
            .ok_or_else(|| anyhow::anyhow!(":query required"))?;
        let limit = extract_int_opt(verb_call, "limit").map(|l| l as usize);
        let include_inactive = extract_bool_opt(verb_call, "include-inactive").unwrap_or(false);

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

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::json!(results));
        }

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

/// Fetch company profile from Companies House
#[register_custom_op]
pub struct ChFetchCompanyOp;

#[async_trait]
impl CustomOperation for ChFetchCompanyOp {
    fn domain(&self) -> &'static str {
        "research.companies-house"
    }
    fn verb(&self) -> &'static str {
        "fetch-company"
    }
    fn rationale(&self) -> &'static str {
        "Fetches company profile from UK Companies House API"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let company_number = extract_string_opt(verb_call, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

        let loader = CompaniesHouseLoader::from_env()?;

        let mut options = FetchOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }

        let entity = loader.fetch_entity(&company_number, Some(options)).await?;
        let result = normalized_entity_to_json(&entity);

        // Log research action if decision-id provided
        if let Some(dec_id) = decision_id {
            log_research_action(pool, dec_id, "companies-house:fetch-company", &result, 0, 0)
                .await?;
        }

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, result.clone());
        }

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({})))
    }
}

/// Fetch PSC (Persons with Significant Control) from Companies House
#[register_custom_op]
pub struct ChFetchPscOp;

#[async_trait]
impl CustomOperation for ChFetchPscOp {
    fn domain(&self) -> &'static str {
        "research.companies-house"
    }
    fn verb(&self) -> &'static str {
        "fetch-psc"
    }
    fn rationale(&self) -> &'static str {
        "Fetches PSC (beneficial owners) from UK Companies House API"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let company_number = extract_string_opt(verb_call, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let include_ceased = extract_bool_opt(verb_call, "include-ceased").unwrap_or(false);
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

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

        // Log research action if decision-id provided
        if let Some(dec_id) = decision_id {
            log_research_action(
                pool,
                dec_id,
                "companies-house:fetch-psc",
                &serde_json::json!({ "holders": results.len() }),
                0,
                0,
            )
            .await?;
        }

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::json!(results));
        }

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

/// Fetch officers (directors, secretaries) from Companies House
#[register_custom_op]
pub struct ChFetchOfficersOp;

#[async_trait]
impl CustomOperation for ChFetchOfficersOp {
    fn domain(&self) -> &'static str {
        "research.companies-house"
    }
    fn verb(&self) -> &'static str {
        "fetch-officers"
    }
    fn rationale(&self) -> &'static str {
        "Fetches officers from UK Companies House API"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let company_number = extract_string_opt(verb_call, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let include_resigned = extract_bool_opt(verb_call, "include-resigned").unwrap_or(false);

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

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::json!(results));
        }

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

/// Import company (and optionally PSC/officers) from Companies House to database
#[register_custom_op]
pub struct ChImportCompanyOp;

#[async_trait]
impl CustomOperation for ChImportCompanyOp {
    fn domain(&self) -> &'static str {
        "research.companies-house"
    }
    fn verb(&self) -> &'static str {
        "import-company"
    }
    fn rationale(&self) -> &'static str {
        "Imports company from Companies House and creates entity in database"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let company_number = extract_string_opt(verb_call, "company-number")
            .ok_or_else(|| anyhow::anyhow!(":company-number required"))?;
        let include_psc = extract_bool_opt(verb_call, "include-psc").unwrap_or(true);
        let include_officers = extract_bool_opt(verb_call, "include-officers").unwrap_or(false);
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

        let loader = CompaniesHouseLoader::from_env()?;

        // Fetch company
        let mut options = FetchOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }
        let entity = loader.fetch_entity(&company_number, Some(options)).await?;

        // Create entity in database
        let entity_id = create_entity_from_normalized(pool, &entity).await?;

        let mut psc_count = 0;
        let mut officer_count = 0;

        // Optionally import PSC
        if include_psc {
            let psc_options = FetchControlHoldersOptions::new();
            let holders = loader
                .fetch_control_holders(&company_number, Some(psc_options))
                .await?;
            psc_count = holders.len();
            // TODO: Create PSC relationships in database
        }

        // Optionally import officers
        if include_officers {
            let officer_options = FetchOfficersOptions::new();
            let officers = loader
                .fetch_officers(&company_number, Some(officer_options))
                .await?;
            officer_count = officers.len();
            // TODO: Create officer relationships in database
        }

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
                pool,
                dec_id,
                "companies-house:import-company",
                &result,
                1,
                0,
            )
            .await?;
        }

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, entity_id);
        }

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "entity_id": uuid::Uuid::now_v7(),
        })))
    }
}

// =============================================================================
// SEC EDGAR Operations (research.sec-edgar domain)
// =============================================================================

/// Search SEC EDGAR for US public companies
#[register_custom_op]
pub struct SecSearchOp;

#[async_trait]
impl CustomOperation for SecSearchOp {
    fn domain(&self) -> &'static str {
        "research.sec-edgar"
    }
    fn verb(&self) -> &'static str {
        "search"
    }
    fn rationale(&self) -> &'static str {
        "Searches SEC EDGAR API for companies by name"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let query = extract_string_opt(verb_call, "query")
            .ok_or_else(|| anyhow::anyhow!(":query required"))?;
        let limit = extract_int_opt(verb_call, "limit").map(|l| l as usize);

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

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::json!(results));
        }

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

/// Fetch company from SEC EDGAR
#[register_custom_op]
pub struct SecFetchCompanyOp;

#[async_trait]
impl CustomOperation for SecFetchCompanyOp {
    fn domain(&self) -> &'static str {
        "research.sec-edgar"
    }
    fn verb(&self) -> &'static str {
        "fetch-company"
    }
    fn rationale(&self) -> &'static str {
        "Fetches company information from SEC EDGAR API"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cik =
            extract_string_opt(verb_call, "cik").ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

        let loader = SecEdgarLoader::new()?;

        let mut options = FetchOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }

        let entity = loader.fetch_entity(&cik, Some(options)).await?;
        let result = normalized_entity_to_json(&entity);

        // Log research action if decision-id provided
        if let Some(dec_id) = decision_id {
            log_research_action(pool, dec_id, "sec-edgar:fetch-company", &result, 0, 0).await?;
        }

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, result.clone());
        }

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({})))
    }
}

/// Fetch 13D/13G beneficial ownership filings from SEC EDGAR
#[register_custom_op]
pub struct SecFetchBeneficialOwnersOp;

#[async_trait]
impl CustomOperation for SecFetchBeneficialOwnersOp {
    fn domain(&self) -> &'static str {
        "research.sec-edgar"
    }
    fn verb(&self) -> &'static str {
        "fetch-beneficial-owners"
    }
    fn rationale(&self) -> &'static str {
        "Fetches 13D/13G beneficial ownership filings from SEC EDGAR API"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cik =
            extract_string_opt(verb_call, "cik").ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let _include_13d = extract_bool_opt(verb_call, "include-13d").unwrap_or(true);
        let _include_13g = extract_bool_opt(verb_call, "include-13g").unwrap_or(true);
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

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

        // Log research action if decision-id provided
        if let Some(dec_id) = decision_id {
            log_research_action(
                pool,
                dec_id,
                "sec-edgar:fetch-beneficial-owners",
                &serde_json::json!({ "holders": results.len() }),
                0,
                0,
            )
            .await?;
        }

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::json!(results));
        }

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

/// Fetch recent SEC filings
#[register_custom_op]
pub struct SecFetchFilingsOp;

#[async_trait]
impl CustomOperation for SecFetchFilingsOp {
    fn domain(&self) -> &'static str {
        "research.sec-edgar"
    }
    fn verb(&self) -> &'static str {
        "fetch-filings"
    }
    fn rationale(&self) -> &'static str {
        "Fetches recent SEC filings from EDGAR API"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cik =
            extract_string_opt(verb_call, "cik").ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let _limit = extract_int_opt(verb_call, "limit").unwrap_or(50);

        // For now, fetch entity and return filing summary from raw_response
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

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, filings.clone());
        }

        Ok(ExecutionResult::Record(
            serde_json::json!({ "filings": filings }),
        ))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(
            serde_json::json!({ "filings": [] }),
        ))
    }
}

/// Import SEC company to database
#[register_custom_op]
pub struct SecImportCompanyOp;

#[async_trait]
impl CustomOperation for SecImportCompanyOp {
    fn domain(&self) -> &'static str {
        "research.sec-edgar"
    }
    fn verb(&self) -> &'static str {
        "import-company"
    }
    fn rationale(&self) -> &'static str {
        "Imports SEC company and creates entity in database"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cik =
            extract_string_opt(verb_call, "cik").ok_or_else(|| anyhow::anyhow!(":cik required"))?;
        let include_beneficial_owners =
            extract_bool_opt(verb_call, "include-beneficial-owners").unwrap_or(true);
        let decision_id = extract_uuid_opt(verb_call, ctx, "decision-id");

        let loader = SecEdgarLoader::new()?;

        // Fetch company
        let mut options = FetchOptions::new();
        if let Some(dec_id) = decision_id {
            options = options.with_decision_id(dec_id);
        }
        let entity = loader.fetch_entity(&cik, Some(options)).await?;

        // Create entity in database
        let entity_id = create_entity_from_normalized(pool, &entity).await?;

        let mut bo_count = 0;

        // Optionally import beneficial owners
        if include_beneficial_owners {
            let bo_options = FetchControlHoldersOptions::new();
            let holders = loader.fetch_control_holders(&cik, Some(bo_options)).await?;
            bo_count = holders.len();
            // TODO: Create BO relationships in database
        }

        let result = serde_json::json!({
            "entity_id": entity_id,
            "cik": cik,
            "name": entity.name,
            "beneficial_owners_imported": bo_count,
        });

        // Log research action if decision-id provided
        if let Some(dec_id) = decision_id {
            log_research_action(pool, dec_id, "sec-edgar:import-company", &result, 1, 0).await?;
        }

        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, entity_id);
        }

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "entity_id": uuid::Uuid::now_v7(),
        })))
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
    let entity_id = Uuid::now_v7();
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
        r#"INSERT INTO kyc.research_actions
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

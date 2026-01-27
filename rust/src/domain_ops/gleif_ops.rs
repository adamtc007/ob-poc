//! GLEIF custom operations
//!
//! Operations for LEI data enrichment and corporate tree import that require
//! GLEIF API calls.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;

use super::helpers::{extract_bool_opt, extract_int_opt, extract_string_opt, extract_uuid_opt};

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};
#[allow(unused_imports)]
use crate::gleif::client::extract_lei_from_url;

#[cfg(feature = "database")]
use {
    crate::dsl_v2::DslExecutor,
    crate::gleif::{
        client::TreeFetchOptions, ChainLink, DiscoveredEntity, FundListResult, GleifClient,
        GleifEnrichmentService, LeiRecord, OwnershipChain, SuccessorResult, UboStatus,
    },
    sqlx::PgPool,
    std::collections::HashMap,
    std::sync::Arc,
    uuid::Uuid,
};

/// Enrich entity from GLEIF by LEI
///
/// Rationale: Requires external GLEIF API call to fetch LEI data.
#[register_custom_op]
pub struct GleifEnrichOp;

#[async_trait]
impl CustomOperation for GleifEnrichOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "enrich"
    }
    fn rationale(&self) -> &'static str {
        "Requires external GLEIF API call to fetch and persist LEI data"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Get LEI or entity-id
        let lei = extract_string_opt(verb_call, "lei");
        let entity_id_arg = verb_call
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

        let (lei, entity_id): (String, Uuid) = match (lei, entity_id_arg) {
            (Some(l), _) => {
                // Look up or create entity by LEI
                let existing: Option<Uuid> = sqlx::query_scalar(
                    r#"SELECT entity_id FROM "ob-poc".entity_limited_companies WHERE lei = $1"#,
                )
                .bind(&l)
                .fetch_optional(pool)
                .await?;

                match existing {
                    Some(id) => (l, id),
                    None => {
                        // Create new entity from GLEIF using DSL (ensures deduplication by name)
                        let client = GleifClient::new()?;

                        // Fetch LEI record first to get entity name
                        let record = client.get_lei_record(&l).await?;
                        let name = &record.attributes.entity.legal_name.name;
                        let jurisdiction = record
                            .attributes
                            .entity
                            .jurisdiction
                            .as_deref()
                            .unwrap_or("XX");

                        // Use DSL to create entity (idempotent by name - prevents duplicates)
                        let dsl = format!(
                            r#"(entity.ensure-limited-company :name "{}" :jurisdiction "{}" :lei "{}" :as @entity)"#,
                            escape_dsl_string(name),
                            jurisdiction,
                            l
                        );

                        let executor = DslExecutor::new(pool.clone());
                        let mut dsl_ctx = ExecutionContext::new();
                        executor.execute_dsl(&dsl, &mut dsl_ctx).await?;

                        // Get the created/existing entity ID from context
                        let entity_id = dsl_ctx
                            .resolve("entity")
                            .ok_or_else(|| anyhow::anyhow!("DSL did not bind @entity"))?;

                        (l, entity_id)
                    }
                }
            }
            (None, Some(eid)) => {
                // Get LEI from existing entity
                let lei: Option<String> = sqlx::query_scalar(
                    r#"SELECT lei FROM "ob-poc".entity_limited_companies WHERE entity_id = $1"#,
                )
                .bind(eid)
                .fetch_optional(pool)
                .await?
                .flatten();

                match lei {
                    Some(l) => (l, eid),
                    None => return Err(anyhow::anyhow!("Entity {} has no LEI", eid)),
                }
            }
            (None, None) => {
                return Err(anyhow::anyhow!("Either :lei or :entity-id required"));
            }
        };

        // Enrich entity with GLEIF data
        let service = GleifEnrichmentService::new(Arc::new(pool.clone()))?;
        let result = service.enrich_entity(entity_id, &lei).await?;

        // Bind result
        if let Some(binding) = &verb_call.binding {
            ctx.bind(binding, entity_id);
        }

        let result_json = serde_json::json!({
            "entity_id": entity_id,
            "lei": lei,
            "names_added": result.names_added,
            "addresses_added": result.addresses_added,
            "identifiers_added": result.identifiers_added,
            "parent_relationships_added": result.parent_relationships_added,
        });

        // Log research action if decision-id provided (links Phase 2 execution to Phase 1 selection)
        if let Some(decision_id) = extract_uuid_opt(verb_call, ctx, "decision-id") {
            let entities_updated = if result.names_added > 0 || result.addresses_added > 0 {
                1
            } else {
                0
            };
            log_research_action(
                pool,
                decision_id,
                "gleif:enrich",
                &result_json,
                0,
                entities_updated,
            )
            .await?;
        }

        Ok(ExecutionResult::Record(result_json))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "entity_id": uuid::Uuid::new_v4(),
            "lei": "MOCK_LEI",
            "names_added": 0,
            "addresses_added": 0,
        })))
    }
}

/// Search GLEIF for entities
///
/// Rationale: Requires external GLEIF API search call.
#[register_custom_op]
pub struct GleifSearchOp;

#[async_trait]
impl CustomOperation for GleifSearchOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "search"
    }
    fn rationale(&self) -> &'static str {
        "Requires external GLEIF API search call"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let name = extract_string_opt(verb_call, "name");
        let limit = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "limit")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(20) as usize;

        let client = GleifClient::new()?;

        let results = match name {
            Some(ref n) => client.search_by_name(n, limit).await?,
            None => return Err(anyhow::anyhow!(":name required for search")),
        };

        let candidates: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "lei": r.lei(),
                    "name": r.attributes.entity.legal_name.name,
                    "jurisdiction": r.attributes.entity.jurisdiction,
                    "category": r.attributes.entity.category,
                    "status": r.attributes.entity.status,
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(candidates))
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

/// Import corporate tree from GLEIF
///
/// Rationale: Requires multiple GLEIF API calls to traverse the corporate structure.
#[register_custom_op]
pub struct GleifImportTreeOp;

#[async_trait]
impl CustomOperation for GleifImportTreeOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "import-tree"
    }
    fn rationale(&self) -> &'static str {
        "Requires multiple GLEIF API calls to traverse and import corporate structure"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let root_lei = extract_string_opt(verb_call, "root-lei")
            .ok_or_else(|| anyhow::anyhow!(":root-lei required"))?;

        let max_depth = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "max-depth")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(3) as usize;

        let service = GleifEnrichmentService::new(Arc::new(pool.clone()))?;
        let result = service.import_corporate_tree(&root_lei, max_depth).await?;

        let result_json = serde_json::json!({
            "root_lei": result.root_lei,
            "entities_created": result.entities_created,
            "entities_updated": result.entities_updated,
            "relationships_created": result.relationships_created,
            "terminal_entities": result.terminal_entities.len(),
        });

        // Log research action if decision-id provided
        if let Some(decision_id) = extract_uuid_opt(verb_call, ctx, "decision-id") {
            log_research_action(
                pool,
                decision_id,
                "gleif:import-tree",
                &result_json,
                result.entities_created as i32,
                result.entities_updated as i32,
            )
            .await?;
        }

        Ok(ExecutionResult::Record(result_json))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "root_lei": "MOCK_LEI",
            "entities_created": 0,
            "entities_updated": 0,
            "relationships_created": 0,
        })))
    }
}

/// Refresh stale GLEIF data
///
/// Rationale: Requires GLEIF API calls to update entity data.
#[register_custom_op]
pub struct GleifRefreshOp;

#[async_trait]
impl CustomOperation for GleifRefreshOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "refresh"
    }
    fn rationale(&self) -> &'static str {
        "Requires GLEIF API calls to refresh stale entity data"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id_arg = verb_call
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

        let stale_days = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "stale-days")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(30) as i32;

        let limit = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "limit")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(100) as i32;

        let service = GleifEnrichmentService::new(Arc::new(pool.clone()))?;

        match entity_id_arg {
            Some(entity_id) => {
                // Refresh single entity
                let result = service.refresh_entity(entity_id).await?;
                Ok(ExecutionResult::Record(serde_json::json!({
                    "refreshed": 1,
                    "entity_id": entity_id,
                    "lei": result.lei,
                })))
            }
            None => {
                // Find stale entities and refresh them
                let stale_entities: Vec<(Uuid, String)> = sqlx::query_as(
                    r#"SELECT entity_id, lei FROM "ob-poc".entity_limited_companies
                       WHERE lei IS NOT NULL
                         AND (gleif_last_update IS NULL
                              OR gleif_last_update < NOW() - $1 * INTERVAL '1 day')
                       LIMIT $2"#,
                )
                .bind(stale_days)
                .bind(limit)
                .fetch_all(pool)
                .await?;

                let mut refreshed = 0;
                let mut errors = 0;

                for (entity_id, lei) in stale_entities {
                    match service.enrich_entity(entity_id, &lei).await {
                        Ok(_) => refreshed += 1,
                        Err(e) => {
                            tracing::warn!("Failed to refresh entity {}: {}", entity_id, e);
                            errors += 1;
                        }
                    }
                }

                Ok(ExecutionResult::Record(serde_json::json!({
                    "refreshed": refreshed,
                    "errors": errors,
                    "stale_days": stale_days,
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
            "refreshed": 0,
            "errors": 0,
        })))
    }
}

/// Get raw GLEIF record (does not persist)
///
/// Rationale: Direct GLEIF API call for inspection.
#[register_custom_op]
pub struct GleifGetRecordOp;

#[async_trait]
impl CustomOperation for GleifGetRecordOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "get-record"
    }
    fn rationale(&self) -> &'static str {
        "Direct GLEIF API call for record inspection"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let lei =
            extract_string_opt(verb_call, "lei").ok_or_else(|| anyhow::anyhow!(":lei required"))?;

        let client = GleifClient::new()?;
        let record = client.get_lei_record(&lei).await?;

        Ok(ExecutionResult::Record(serde_json::json!({
            "lei": record.lei(),
            "name": record.attributes.entity.legal_name.name,
            "jurisdiction": record.attributes.entity.jurisdiction,
            "category": record.attributes.entity.category,
            "sub_category": record.attributes.entity.sub_category,
            "status": record.attributes.entity.status,
            "legal_form": record.attributes.entity.legal_form,
            "registration": record.attributes.registration,
        })))
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

/// Get direct parent from GLEIF
///
/// Rationale: Direct GLEIF API call for parent relationship.
#[register_custom_op]
pub struct GleifGetParentOp;

#[async_trait]
impl CustomOperation for GleifGetParentOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "get-parent"
    }
    fn rationale(&self) -> &'static str {
        "Direct GLEIF API call for parent relationship"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let lei =
            extract_string_opt(verb_call, "lei").ok_or_else(|| anyhow::anyhow!(":lei required"))?;

        let client = GleifClient::new()?;
        let parent = client.get_direct_parent(&lei).await?;

        match parent {
            Some(rel) => Ok(ExecutionResult::Record(serde_json::json!({
                "parent_lei": rel.attributes.relationship.end_node.id,
                "relationship_type": rel.attributes.relationship.relationship_type,
                "relationship_status": rel.attributes.relationship.status,
            }))),
            None => Ok(ExecutionResult::Record(serde_json::json!({
                "parent_lei": null,
                "message": "No direct parent found"
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
            "parent_lei": null,
        })))
    }
}

/// Import managed funds from GLEIF with full CBU structure
///
/// Rationale: Fetches funds from GLEIF API and creates entities + CBUs with role assignments.
#[register_custom_op]
pub struct GleifImportManagedFundsOp;

#[async_trait]
impl CustomOperation for GleifImportManagedFundsOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "import-managed-funds"
    }
    fn rationale(&self) -> &'static str {
        "Fetches managed funds from GLEIF API and creates entities + CBUs with role assignments"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let manager_lei = extract_string_opt(verb_call, "manager-lei");
        let name_pattern = extract_string_opt(verb_call, "name-pattern");

        // Either manager-lei or name-pattern is required
        if manager_lei.is_none() && name_pattern.is_none() {
            return Err(anyhow::anyhow!(":manager-lei or :name-pattern required"));
        }

        let ultimate_client_lei = extract_string_opt(verb_call, "ultimate-client-lei");

        let create_cbus = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "create-cbus")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);

        let limit = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "limit")
            .and_then(|a| a.value.as_integer())
            .map(|l| l as usize)
            .unwrap_or(1000); // Default limit to prevent runaway imports

        let dry_run = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "dry-run")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

        let client = GleifClient::new()?;

        // Fetch funds - try relationship endpoint first, then fall back to name search
        let mut funds = Vec::new();
        let mut search_method = "relationship";

        if let Some(ref lei) = manager_lei {
            tracing::info!("Fetching managed funds for manager LEI: {}", lei);
            funds = client.get_managed_funds(lei).await?;

            // If relationship endpoint returns nothing, try name-based search
            if funds.is_empty() {
                if let Some(ref pattern) = name_pattern {
                    tracing::info!(
                        "Relationship endpoint empty, falling back to name search: {}",
                        pattern
                    );
                    funds = client.search_funds_by_name(pattern, limit).await?;
                    search_method = "name_search";
                } else {
                    // Try to get manager name for fallback search
                    let manager_record = client.get_lei_record(lei).await?;
                    let manager_name = &manager_record.attributes.entity.legal_name.name;

                    // Extract a searchable prefix (e.g., "Allianz" from "Allianz Global Investors GmbH")
                    let search_prefix = manager_name
                        .split_whitespace()
                        .next()
                        .unwrap_or(manager_name);
                    tracing::info!(
                        "Relationship endpoint empty, falling back to name search with prefix: {}",
                        search_prefix
                    );
                    funds = client.search_funds_by_name(search_prefix, limit).await?;
                    search_method = "name_search_auto";
                }
            }
        } else if let Some(ref pattern) = name_pattern {
            tracing::info!("Searching funds by name pattern: {}", pattern);
            funds = client.search_funds_by_name(pattern, limit).await?;
            search_method = "name_search";
        }

        // Apply limit
        funds.truncate(limit);

        tracing::info!(
            "Found {} funds to import via {}",
            funds.len(),
            search_method
        );

        if dry_run {
            let fund_info: Vec<serde_json::Value> = funds
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "lei": f.lei(),
                        "name": f.attributes.entity.legal_name.name,
                        "jurisdiction": f.attributes.entity.jurisdiction,
                        "category": f.attributes.entity.category,
                    })
                })
                .collect();

            return Ok(ExecutionResult::Record(serde_json::json!({
                "dry_run": true,
                "manager_lei": manager_lei,
                "name_pattern": name_pattern,
                "search_method": search_method,
                "funds_found": funds.len(),
                "funds": fund_info,
            })));
        }

        // Get or create manager entity (only if manager_lei provided)
        let manager_entity_id = if let Some(ref lei) = manager_lei {
            Some(get_or_create_entity_by_lei(pool, &client, lei).await?)
        } else {
            None
        };

        // Get or create ultimate client entity (if provided)
        let ultimate_client_entity_id = if let Some(ref uc_lei) = ultimate_client_lei {
            Some(get_or_create_entity_by_lei(pool, &client, uc_lei).await?)
        } else {
            None
        };

        let mut entities_created = 0;
        let mut cbus_created = 0;
        let mut roles_assigned = 0;

        for fund in &funds {
            let fund_lei = &fund.lei();
            let fund_name = &fund.attributes.entity.legal_name.name;
            let jurisdiction = fund
                .attributes
                .entity
                .jurisdiction
                .as_deref()
                .unwrap_or("LU");

            // Get or create fund entity
            let fund_entity_id = get_or_create_entity_from_record(pool, fund).await?;
            entities_created += 1;

            if create_cbus {
                // Create CBU for the fund
                let cbu_id = create_fund_cbu(pool, fund_name, jurisdiction).await?;
                cbus_created += 1;

                // Assign ASSET_OWNER role (fund owns itself)
                assign_role(pool, cbu_id, fund_entity_id, "ASSET_OWNER").await?;
                roles_assigned += 1;

                // Assign INVESTMENT_MANAGER and MANAGEMENT_COMPANY roles if manager known
                if let Some(mgr_id) = manager_entity_id {
                    assign_role(pool, cbu_id, mgr_id, "INVESTMENT_MANAGER").await?;
                    roles_assigned += 1;

                    // Assign MANAGEMENT_COMPANY role (same as IM for self-managed)
                    assign_role(pool, cbu_id, mgr_id, "MANAGEMENT_COMPANY").await?;
                    roles_assigned += 1;
                }

                // Assign ULTIMATE_CLIENT role if provided
                if let Some(uc_id) = ultimate_client_entity_id {
                    assign_role(pool, cbu_id, uc_id, "ULTIMATE_CLIENT").await?;
                    roles_assigned += 1;
                }

                // Check for umbrella fund relationship
                if let Some(ref rels) = fund.relationships {
                    if let Some(ref umbrella) = rels.umbrella_fund {
                        if let Some(ref url) = umbrella.links.related {
                            if let Some(umbrella_lei) = url.split('/').next_back() {
                                // Get or create umbrella entity
                                let umbrella_entity_id =
                                    get_or_create_entity_by_lei(pool, &client, umbrella_lei)
                                        .await?;
                                // Assign SICAV role (umbrella is the SICAV)
                                assign_role(pool, cbu_id, umbrella_entity_id, "SICAV").await?;
                                roles_assigned += 1;
                            }
                        }
                    }
                }
            }

            tracing::debug!("Imported fund: {} ({})", fund_name, fund_lei);
        }

        let result_json = serde_json::json!({
            "manager_lei": manager_lei,
            "name_pattern": name_pattern,
            "search_method": search_method,
            "funds_imported": funds.len(),
            "entities_created": entities_created,
            "cbus_created": cbus_created,
            "roles_assigned": roles_assigned,
        });

        // Log research action if decision-id provided
        if let Some(decision_id) = extract_uuid_opt(verb_call, _ctx, "decision-id") {
            log_research_action(
                pool,
                decision_id,
                "gleif:import-managed-funds",
                &result_json,
                entities_created,
                0,
            )
            .await?;
        }

        Ok(ExecutionResult::Record(result_json))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "dry_run": true,
            "funds_imported": 0,
        })))
    }
}

// Helper functions for GleifImportManagedFundsOp
#[cfg(feature = "database")]
async fn get_or_create_entity_by_lei(
    pool: &PgPool,
    client: &GleifClient,
    lei: &str,
) -> Result<Uuid> {
    // Check if entity exists in entity_funds first (for FUND category)
    let existing: Option<Uuid> =
        sqlx::query_scalar(r#"SELECT entity_id FROM "ob-poc".entity_funds WHERE lei = $1"#)
            .bind(lei)
            .fetch_optional(pool)
            .await?;

    if let Some(id) = existing {
        return Ok(id);
    }

    // Also check entity_limited_companies (for non-fund entities like ManCos)
    let existing: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT entity_id FROM "ob-poc".entity_limited_companies WHERE lei = $1"#,
    )
    .bind(lei)
    .fetch_optional(pool)
    .await?;

    if let Some(id) = existing {
        return Ok(id);
    }

    // Fetch from GLEIF and create
    let record = client.get_lei_record(lei).await?;
    get_or_create_entity_from_record(pool, &record).await
}

#[cfg(feature = "database")]
async fn get_or_create_entity_from_record(pool: &PgPool, record: &LeiRecord) -> Result<Uuid> {
    let lei = record.lei();
    let name = &record.attributes.entity.legal_name.name;
    let jurisdiction = record
        .attributes
        .entity
        .jurisdiction
        .as_deref()
        .unwrap_or("XX");
    let category = record.attributes.entity.category.as_deref();

    let is_fund = category == Some("FUND");

    // Check if entity exists by LEI first (fast path - no DSL needed)
    if is_fund {
        let existing: Option<Uuid> =
            sqlx::query_scalar(r#"SELECT entity_id FROM "ob-poc".entity_funds WHERE lei = $1"#)
                .bind(lei)
                .fetch_optional(pool)
                .await?;

        if let Some(id) = existing {
            return Ok(id);
        }
    } else {
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT entity_id FROM "ob-poc".entity_limited_companies WHERE lei = $1"#,
        )
        .bind(lei)
        .fetch_optional(pool)
        .await?;

        if let Some(id) = existing {
            return Ok(id);
        }
    }

    // Create entity using DSL (idempotent by name - prevents duplicates)
    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ExecutionContext::new();

    let dsl = if is_fund {
        // Use fund.ensure-standalone for fund entities (idempotent via upsert, minimal required fields)
        format!(
            r#"(fund.ensure-standalone :name "{}" :jurisdiction "{}" :lei "{}" :as @entity)"#,
            escape_dsl_string(name),
            jurisdiction,
            lei
        )
    } else {
        // Use entity.ensure-limited-company for non-fund entities
        format!(
            r#"(entity.ensure-limited-company :name "{}" :jurisdiction "{}" :lei "{}" :as @entity)"#,
            escape_dsl_string(name),
            jurisdiction,
            lei
        )
    };

    executor.execute_dsl(&dsl, &mut ctx).await?;

    // Get the created/existing entity ID from context
    let entity_id = ctx
        .resolve("entity")
        .ok_or_else(|| anyhow::anyhow!("DSL did not bind @entity for {}", name))?;

    Ok(entity_id)
}

#[cfg(feature = "database")]
async fn create_fund_cbu(pool: &PgPool, name: &str, jurisdiction: &str) -> Result<Uuid> {
    // Use DSL to create CBU (idempotent by name - prevents duplicates)
    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ExecutionContext::new();

    let dsl = format!(
        r#"(cbu.ensure :name "{}" :jurisdiction "{}" :client-type "FUND" :as @cbu)"#,
        escape_dsl_string(name),
        jurisdiction
    );

    executor.execute_dsl(&dsl, &mut ctx).await?;

    let cbu_id = ctx
        .resolve("cbu")
        .ok_or_else(|| anyhow::anyhow!("DSL did not bind @cbu for {}", name))?;

    Ok(cbu_id)
}

#[cfg(feature = "database")]
async fn assign_role(pool: &PgPool, cbu_id: Uuid, entity_id: Uuid, role_name: &str) -> Result<()> {
    // Use DSL to assign role (idempotent - prevents duplicates)
    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ExecutionContext::new();

    let dsl = format!(
        r#"(cbu.assign-role :cbu-id "{}" :entity-id "{}" :role "{}")"#,
        cbu_id, entity_id, role_name
    );

    // Execute and ignore "already assigned" errors
    match executor.execute_dsl(&dsl, &mut ctx).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let err_str = e.to_string();
            // Ignore duplicate assignment errors
            if err_str.contains("already assigned") || err_str.contains("duplicate") {
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

/// Get direct children from GLEIF
///
/// Rationale: Direct GLEIF API call for child entities.
#[register_custom_op]
pub struct GleifGetChildrenOp;

#[async_trait]
impl CustomOperation for GleifGetChildrenOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "get-children"
    }
    fn rationale(&self) -> &'static str {
        "Direct GLEIF API call for child entities"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let lei =
            extract_string_opt(verb_call, "lei").ok_or_else(|| anyhow::anyhow!(":lei required"))?;

        let client = GleifClient::new()?;
        let children = client.get_direct_children(&lei).await?;

        let results: Vec<serde_json::Value> = children
            .iter()
            .map(|r| {
                serde_json::json!({
                    "lei": r.lei(),
                    "name": r.attributes.entity.legal_name.name,
                    "jurisdiction": r.attributes.entity.jurisdiction,
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

/// Trace ownership chain to UBO terminus
#[register_custom_op]
pub struct GleifTraceOwnershipOp;

#[async_trait]
impl CustomOperation for GleifTraceOwnershipOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "trace-ownership"
    }
    fn rationale(&self) -> &'static str {
        "Follows parent relationships to UBO terminus via GLEIF API"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let lei = match extract_string_opt(verb_call, "lei") {
            Some(l) => l,
            None => {
                // Try to get LEI from entity-id
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
                    })
                    .ok_or_else(|| anyhow::anyhow!(":lei or :entity-id required"))?;

                get_lei_for_entity(pool, entity_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Entity {} has no LEI", entity_id))?
            }
        };

        let client = GleifClient::new()?;

        // Get starting entity info
        let start_record = client.get_lei_record(&lei).await?;
        let start_name = start_record.attributes.entity.legal_name.name.clone();

        // Trace parent chain
        let mut chain = Vec::new();
        let mut current_lei = lei.clone();
        let mut terminus = UboStatus::Unknown;

        for _depth in 0..10 {
            match client.get_direct_parent(&current_lei).await? {
                Some(rel) => {
                    let parent_lei = rel.attributes.relationship.end_node.id.clone();
                    let parent_record = client.get_lei_record(&parent_lei).await?;

                    chain.push(ChainLink {
                        lei: parent_lei.clone(),
                        name: parent_record.attributes.entity.legal_name.name.clone(),
                        jurisdiction: parent_record.attributes.entity.jurisdiction.clone(),
                        relationship_type: rel.attributes.relationship.relationship_type.clone(),
                        corroboration_level: rel.attributes.registration.validation_sources.clone(),
                    });

                    current_lei = parent_lei;
                }
                None => {
                    // No parent - check for reporting exception
                    // For now, assume public float if no parent
                    terminus = UboStatus::PublicFloat;
                    break;
                }
            }
        }

        let result = OwnershipChain {
            start_lei: lei,
            start_name,
            chain: chain.clone(),
            terminus: terminus.clone(),
            total_depth: chain.len(),
        };

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::to_value(&result)?);
        }

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "chain": [],
            "terminus": "Unknown",
        })))
    }
}

/// Get all funds managed by an investment manager
#[register_custom_op]
pub struct GleifGetManagedFundsOp;

#[async_trait]
impl CustomOperation for GleifGetManagedFundsOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "get-managed-funds"
    }
    fn rationale(&self) -> &'static str {
        "Fetches all funds managed by an investment manager from GLEIF"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let manager_lei = extract_string_opt(verb_call, "manager-lei")
            .ok_or_else(|| anyhow::anyhow!(":manager-lei required"))?;
        let resolve_umbrellas = extract_bool_opt(verb_call, "resolve-umbrellas").unwrap_or(true);
        let limit = extract_int_opt(verb_call, "limit");

        let client = GleifClient::new()?;

        // Get manager name
        let manager_record = client.get_lei_record(&manager_lei).await?;
        let manager_name = manager_record.attributes.entity.legal_name.name.clone();

        // Fetch managed funds
        let mut all_funds = client.get_managed_funds(&manager_lei).await?;

        if let Some(lim) = limit {
            all_funds.truncate(lim as usize);
        }

        let funds: Vec<DiscoveredEntity> = all_funds
            .iter()
            .map(DiscoveredEntity::from_lei_record)
            .collect();

        // Resolve umbrellas
        let mut fund_umbrellas: HashMap<String, DiscoveredEntity> = HashMap::new();
        if resolve_umbrellas {
            for fund in &funds {
                if let Ok(Some(umbrella)) = client.get_umbrella_fund(&fund.lei).await {
                    fund_umbrellas.insert(
                        fund.lei.clone(),
                        DiscoveredEntity::from_lei_record(&umbrella),
                    );
                }
            }
        }

        let result = FundListResult {
            manager_lei: manager_lei.clone(),
            manager_name: Some(manager_name),
            funds: funds.clone(),
            fund_umbrellas,
            total_count: funds.len(),
        };

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::to_value(&result)?);
        }

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "funds": [],
            "total_count": 0,
        })))
    }
}

/// Resolve merged/inactive LEI to current successor
#[register_custom_op]
pub struct GleifResolveSuccessorOp;

#[async_trait]
impl CustomOperation for GleifResolveSuccessorOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "resolve-successor"
    }
    fn rationale(&self) -> &'static str {
        "Follows successor chain for merged/inactive entities"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let lei =
            extract_string_opt(verb_call, "lei").ok_or_else(|| anyhow::anyhow!(":lei required"))?;

        let client = GleifClient::new()?;

        let mut current_lei = lei.clone();
        let mut chain = vec![current_lei.clone()];
        let mut was_merged = false;

        loop {
            let record = client.get_lei_record(&current_lei).await?;

            let status = record
                .attributes
                .entity
                .status
                .as_deref()
                .unwrap_or("ACTIVE");
            if status == "ACTIVE" {
                break;
            }

            was_merged = true;

            // Check for successor
            if let Some(successor) = record.attributes.entity.successor_entities.first() {
                chain.push(successor.lei.clone());
                current_lei = successor.lei.clone();
            } else {
                break;
            }

            if chain.len() > 10 {
                break;
            }
        }

        let final_record = client.get_lei_record(&current_lei).await?;

        let result = SuccessorResult {
            original_lei: lei,
            current_lei: current_lei.clone(),
            chain,
            current_entity: DiscoveredEntity::from_lei_record(&final_record),
            was_merged,
        };

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::to_value(&result)?);
        }

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "was_merged": false,
        })))
    }
}

// =============================================================================
// Fund Structure Relationship Verbs (Lean GLEIF API)
// =============================================================================

/// Get umbrella fund for a sub-fund (IS_SUBFUND_OF relationship)
///
/// Single deterministic lookup - returns the umbrella fund that a sub-fund belongs to.
/// SICAVs are self-governed and have no umbrella - use get-manager to find ManCo instead.
#[register_custom_op]
pub struct GleifGetUmbrellaOp;

#[async_trait]
impl CustomOperation for GleifGetUmbrellaOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "get-umbrella"
    }
    fn rationale(&self) -> &'static str {
        "Single GLEIF API lookup for IS_SUBFUND_OF relationship"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::gleif::{UmbrellaEntity, UmbrellaResult};

        let lei = match extract_string_opt(verb_call, "lei") {
            Some(l) => l,
            None => {
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
                    })
                    .ok_or_else(|| anyhow::anyhow!(":lei or :entity-id required"))?;

                get_lei_for_entity(pool, entity_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Entity {} has no LEI", entity_id))?
            }
        };

        let client = GleifClient::new()?;

        // Get the sub-fund record for its name
        let subfund = client.get_lei_record(&lei).await?;
        let subfund_name = subfund.attributes.entity.legal_name.name.clone();

        // Look up umbrella fund
        let umbrella = client.get_umbrella_fund(&lei).await?;

        let result = UmbrellaResult {
            subfund_lei: lei,
            subfund_name,
            umbrella: umbrella.map(|u| UmbrellaEntity {
                lei: u.lei().to_string(),
                name: u.attributes.entity.legal_name.name,
                jurisdiction: u.attributes.entity.jurisdiction,
            }),
        };

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::to_value(&result)?);
        }

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "subfund_lei": "MOCK",
            "subfund_name": "Mock Fund",
            "umbrella": null,
        })))
    }
}

/// Get fund manager for a fund (IS_FUND-MANAGED_BY relationship)
///
/// Single deterministic lookup - returns the ManCo/AIFM/IM that manages the fund.
/// This is the correct starting point for SICAVs which have no umbrella above them.
#[register_custom_op]
pub struct GleifGetManagerOp;

#[async_trait]
impl CustomOperation for GleifGetManagerOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "get-manager"
    }
    fn rationale(&self) -> &'static str {
        "Single GLEIF API lookup for IS_FUND-MANAGED_BY relationship"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::gleif::{ManagerEntity, ManagerResult};

        let lei = match extract_string_opt(verb_call, "lei") {
            Some(l) => l,
            None => {
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
                    })
                    .ok_or_else(|| anyhow::anyhow!(":lei or :entity-id required"))?;

                get_lei_for_entity(pool, entity_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Entity {} has no LEI", entity_id))?
            }
        };

        let client = GleifClient::new()?;

        // Get the fund record for its name
        let fund = client.get_lei_record(&lei).await?;
        let fund_name = fund.attributes.entity.legal_name.name.clone();

        // Look up fund manager
        let manager = client.get_fund_manager(&lei).await?;

        let result = ManagerResult {
            fund_lei: lei,
            fund_name,
            manager: manager.map(|m| ManagerEntity {
                lei: m.lei().to_string(),
                name: m.attributes.entity.legal_name.name,
                jurisdiction: m.attributes.entity.jurisdiction,
                role: "FUND_MANAGER".to_string(),
            }),
        };

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::to_value(&result)?);
        }

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "fund_lei": "MOCK",
            "fund_name": "Mock Fund",
            "manager": null,
        })))
    }
}

/// Get master fund for a feeder fund (IS_FEEDER_TO relationship)
///
/// Single deterministic lookup - returns the master fund that a feeder invests in.
#[register_custom_op]
pub struct GleifGetMasterFundOp;

#[async_trait]
impl CustomOperation for GleifGetMasterFundOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "get-master-fund"
    }
    fn rationale(&self) -> &'static str {
        "Single GLEIF API lookup for IS_FEEDER_TO relationship"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::gleif::{MasterEntity, MasterFundResult};

        let lei = match extract_string_opt(verb_call, "lei") {
            Some(l) => l,
            None => {
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
                    })
                    .ok_or_else(|| anyhow::anyhow!(":lei or :entity-id required"))?;

                get_lei_for_entity(pool, entity_id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Entity {} has no LEI", entity_id))?
            }
        };

        let client = GleifClient::new()?;

        // Get the feeder fund record for its name
        let feeder = client.get_lei_record(&lei).await?;
        let feeder_name = feeder.attributes.entity.legal_name.name.clone();

        // Look up master fund
        let master = client.get_master_fund(&lei).await?;

        let result = MasterFundResult {
            feeder_lei: lei,
            feeder_name,
            master: master.map(|m| MasterEntity {
                lei: m.lei().to_string(),
                name: m.attributes.entity.legal_name.name,
                jurisdiction: m.attributes.entity.jurisdiction,
            }),
        };

        if let Some(binding) = &verb_call.binding {
            ctx.bind_json(binding, serde_json::to_value(&result)?);
        }

        Ok(ExecutionResult::Record(serde_json::to_value(&result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "feeder_lei": "MOCK",
            "feeder_name": "Mock Feeder",
            "master": null,
        })))
    }
}

/// Look up entity LEI by ISIN
///
/// Single deterministic lookup - given an ISIN, returns the issuing entity's LEI.
#[register_custom_op]
pub struct GleifLookupByIsinOp;

#[async_trait]
impl CustomOperation for GleifLookupByIsinOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "lookup-by-isin"
    }
    fn rationale(&self) -> &'static str {
        "Single GLEIF API lookup for ISIN to LEI mapping"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::gleif::IsinLookupResult;

        let isin = extract_string_opt(verb_call, "isin")
            .ok_or_else(|| anyhow::anyhow!(":isin required"))?;

        let client = GleifClient::new()?;
        let record = client.lookup_by_isin(&isin).await?;

        let result = match record {
            Some(r) => serde_json::to_value(&IsinLookupResult {
                isin,
                lei: r.lei().to_string(),
                name: r.attributes.entity.legal_name.name,
                jurisdiction: r.attributes.entity.jurisdiction,
            })?,
            None => serde_json::json!({
                "isin": isin,
                "lei": null,
                "name": null,
                "jurisdiction": null,
                "message": "No LEI found for ISIN"
            }),
        };

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
        Ok(ExecutionResult::Record(serde_json::json!({
            "isin": "MOCK",
            "lei": null,
        })))
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get LEI for an entity by looking up entity_limited_companies
#[cfg(feature = "database")]
async fn get_lei_for_entity(pool: &PgPool, entity_id: Uuid) -> Result<Option<String>> {
    let lei: Option<String> = sqlx::query_scalar(
        r#"SELECT lei FROM "ob-poc".entity_limited_companies WHERE entity_id = $1"#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?
    .flatten();

    Ok(lei)
}

/// Log a research action to the audit trail when decision-id is provided.
/// This links Phase 2 DSL execution back to Phase 1 LLM selection.
#[cfg(feature = "database")]
async fn log_research_action(
    pool: &PgPool,
    decision_id: Uuid,
    verb_fqn: &str,
    result_summary: &serde_json::Value,
    entities_created: i32,
    entities_updated: i32,
) -> Result<Uuid> {
    let action_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO kyc.research_actions
           (decision_id, verb_fqn, result_summary, entities_created, entities_updated)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING action_id"#,
    )
    .bind(decision_id)
    .bind(verb_fqn)
    .bind(result_summary)
    .bind(entities_created)
    .bind(entities_updated)
    .fetch_one(pool)
    .await?;

    Ok(action_id)
}

/// Escape a string for use in DSL - handles quotes and special characters
#[cfg(feature = "database")]
fn escape_dsl_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// =============================================================================
// GLEIF IMPORT TO CLIENT GROUP
// =============================================================================

/// Import GLEIF tree and populate client_group_entity with role tagging
///
/// This operation:
/// 1. Creates/finds the client_group
/// 2. Imports the GLEIF corporate tree
/// 3. Adds all discovered entities to client_group_entity
/// 4. Auto-tags entities with roles based on GLEIF category/relationships
/// 5. Creates client_group_relationship edges
/// 6. Records source provenance
#[register_custom_op]
pub struct GleifImportToClientGroupOp;

#[async_trait]
impl CustomOperation for GleifImportToClientGroupOp {
    fn domain(&self) -> &'static str {
        "gleif"
    }
    fn verb(&self) -> &'static str {
        "import-to-client-group"
    }
    fn rationale(&self) -> &'static str {
        "Imports GLEIF tree and populates client_group tables with role tagging"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let group_id = super::helpers::get_required_uuid(verb_call, "group-id")?;
        let root_lei = extract_string_opt(verb_call, "root-lei")
            .ok_or_else(|| anyhow::anyhow!(":root-lei required"))?;
        let max_depth = extract_int_opt(verb_call, "max-depth").unwrap_or(3) as usize;

        // New: fund inclusion options
        let include_funds = extract_bool_opt(verb_call, "include-funds").unwrap_or(false);
        let max_funds_per_manco =
            extract_int_opt(verb_call, "max-funds-per-manco").map(|v| v as usize);

        // Start discovery status
        sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group
            SET discovery_status = 'in_progress',
                discovery_started_at = NOW(),
                discovery_source = 'gleif',
                discovery_root_lei = $2,
                updated_at = NOW()
            WHERE id = $1
            "#,
            group_id,
            &root_lei
        )
        .execute(pool)
        .await?;

        // Import the GLEIF tree (creates/updates entities)
        let service = GleifEnrichmentService::new(Arc::new(pool.clone()))?;

        // Use enhanced traversal if including funds, otherwise basic traversal
        let tree_result = if include_funds {
            let options = TreeFetchOptions {
                max_depth,
                include_managed_funds: true,
                include_fund_structures: true,
                include_master_feeder: true,
                fund_type_filter: vec![],
                fund_jurisdiction_filter: vec![],
                max_funds_per_manco: max_funds_per_manco.or(Some(500)),
            };
            service
                .import_corporate_tree_with_options(&root_lei, options)
                .await?
        } else {
            service.import_corporate_tree(&root_lei, max_depth).await?
        };

        // Get all entities with LEIs that were part of this import
        // We use the imported_leis from tree_result to ensure we get exactly the entities
        // that were just imported, regardless of timing
        let imported_leis = &tree_result.imported_leis;

        let entities_with_lei: Vec<(Uuid, String, String, Option<String>)> =
            if imported_leis.is_empty() {
                vec![]
            } else {
                sqlx::query_as(
                    r#"
                SELECT
                    e.entity_id,
                    e.name,
                    COALESCE(elc.lei, ef.lei) as lei,
                    COALESCE(elc.gleif_category, ef.gleif_category) as category
                FROM "ob-poc".entities e
                LEFT JOIN "ob-poc".entity_limited_companies elc ON elc.entity_id = e.entity_id
                LEFT JOIN "ob-poc".entity_funds ef ON ef.entity_id = e.entity_id
                WHERE COALESCE(elc.lei, ef.lei) = ANY($1)
                "#,
                )
                .bind(imported_leis)
                .fetch_all(pool)
                .await?
            };

        let mut entities_added = 0i64;
        let mut funds_added = 0i64;
        let mut roles_assigned = 0i64;
        let mut relationships_created = 0i64;

        // Build lookup of fund LEIs -> ManCo LEI from discovered relationships
        // This tells us which funds are IM-related and who manages them
        let mut fund_to_manco: HashMap<String, String> = HashMap::new();
        let mut fund_leis: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Query entity_parent_relationships to find FUND_MANAGER relationships
        let fund_manager_rels: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT
                COALESCE(elc_child.lei, ef_child.lei) as fund_lei,
                pr.parent_lei as manco_lei
            FROM "ob-poc".entity_parent_relationships pr
            JOIN "ob-poc".entities e ON e.entity_id = pr.child_entity_id
            LEFT JOIN "ob-poc".entity_limited_companies elc_child ON elc_child.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_funds ef_child ON ef_child.entity_id = e.entity_id
            WHERE pr.relationship_type = 'FUND_MANAGER'
              AND pr.relationship_status = 'ACTIVE'
              AND COALESCE(elc_child.lei, ef_child.lei) = ANY($1)
            "#,
        )
        .bind(imported_leis)
        .fetch_all(pool)
        .await?;

        for (fund_lei, manco_lei) in fund_manager_rels {
            fund_to_manco.insert(fund_lei.clone(), manco_lei);
            fund_leis.insert(fund_lei);
        }

        // Get role IDs we'll need
        let role_map: HashMap<String, Uuid> = sqlx::query_as::<_, (String, Uuid)>(
            r#"
            SELECT name, role_id
            FROM "ob-poc".roles
            WHERE name IN (
                'FUND', 'SICAV', 'UCITS', 'AIF', 'HOLDING_COMPANY',
                'ULTIMATE_PARENT', 'SUBSIDIARY', 'BRANCH', 'SPV'
            )
            "#,
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect();

        for (entity_id, entity_name, lei, category_str) in &entities_with_lei {
            let is_fund = category_str.as_deref() == Some("FUND");
            let manco_lei = fund_to_manco.get(lei);

            // Determine relationship category based on how this entity was discovered
            let (relationship_category, added_by) = if is_fund && manco_lei.is_some() {
                // Fund discovered via IM relationship
                ("INVESTMENT_MANAGEMENT", "gleif_im")
            } else {
                // Ownership hierarchy
                ("OWNERSHIP", "gleif")
            };

            // Add to client_group_entity (upsert) with new columns
            let cge_id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO "ob-poc".client_group_entity (
                    group_id, entity_id, membership_type, added_by, source_record_id,
                    relationship_category, is_fund, related_via_lei
                )
                VALUES ($1, $2, 'in_group', $3, $4, $5, $6, $7)
                ON CONFLICT (group_id, entity_id)
                DO UPDATE SET
                    updated_at = NOW(),
                    source_record_id = EXCLUDED.source_record_id,
                    relationship_category = EXCLUDED.relationship_category,
                    is_fund = EXCLUDED.is_fund,
                    related_via_lei = COALESCE(EXCLUDED.related_via_lei, "ob-poc".client_group_entity.related_via_lei)
                RETURNING id
                "#,
            )
            .bind(group_id)
            .bind(entity_id)
            .bind(added_by)
            .bind(lei)
            .bind(relationship_category)
            .bind(is_fund)
            .bind(manco_lei)
            .fetch_one(pool)
            .await?;

            entities_added += 1;
            if is_fund {
                funds_added += 1;

                // Populate fund_metadata for fund entities
                // Look up umbrella/master relationships from entity_parent_relationships
                let umbrella_lei: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT parent_lei
                    FROM "ob-poc".entity_parent_relationships
                    WHERE child_entity_id = $1
                      AND relationship_type = 'UMBRELLA_FUND'
                      AND relationship_status = 'ACTIVE'
                    LIMIT 1
                    "#,
                )
                .bind(entity_id)
                .fetch_optional(pool)
                .await?
                .flatten();

                let master_fund_lei: Option<String> = sqlx::query_scalar(
                    r#"
                    SELECT parent_lei
                    FROM "ob-poc".entity_parent_relationships
                    WHERE child_entity_id = $1
                      AND relationship_type = 'MASTER_FUND'
                      AND relationship_status = 'ACTIVE'
                    LIMIT 1
                    "#,
                )
                .bind(entity_id)
                .fetch_optional(pool)
                .await?
                .flatten();

                // Look up ManCo entity_id if we have a ManCo LEI
                let manco_entity_id: Option<Uuid> = if let Some(ref m_lei) = manco_lei {
                    sqlx::query_scalar(
                        r#"
                        SELECT entity_id
                        FROM "ob-poc".entity_limited_companies
                        WHERE lei = $1
                        LIMIT 1
                        "#,
                    )
                    .bind(m_lei)
                    .fetch_optional(pool)
                    .await?
                } else {
                    None
                };

                // Get ManCo name for denormalized storage
                let manco_name: Option<String> = if let Some(manco_eid) = manco_entity_id {
                    sqlx::query_scalar(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                        .bind(manco_eid)
                        .fetch_optional(pool)
                        .await?
                } else {
                    None
                };

                // Upsert fund_metadata
                sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".fund_metadata (
                        entity_id, lei, umbrella_lei, is_umbrella,
                        master_fund_lei, is_feeder, is_master,
                        manco_lei, manco_name, manco_entity_id,
                        source, updated_at
                    )
                    VALUES ($1, $2, $3, FALSE, $4, $5, FALSE, $6, $7, $8, 'gleif', NOW())
                    ON CONFLICT (entity_id)
                    DO UPDATE SET
                        umbrella_lei = COALESCE(EXCLUDED.umbrella_lei, "ob-poc".fund_metadata.umbrella_lei),
                        master_fund_lei = COALESCE(EXCLUDED.master_fund_lei, "ob-poc".fund_metadata.master_fund_lei),
                        is_feeder = EXCLUDED.is_feeder,
                        manco_lei = COALESCE(EXCLUDED.manco_lei, "ob-poc".fund_metadata.manco_lei),
                        manco_name = COALESCE(EXCLUDED.manco_name, "ob-poc".fund_metadata.manco_name),
                        manco_entity_id = COALESCE(EXCLUDED.manco_entity_id, "ob-poc".fund_metadata.manco_entity_id),
                        updated_at = NOW()
                    "#,
                )
                .bind(entity_id)
                .bind(lei)
                .bind(&umbrella_lei)
                .bind(&master_fund_lei)
                .bind(master_fund_lei.is_some()) // is_feeder = true if has master fund
                .bind(manco_lei.as_ref())
                .bind(&manco_name)
                .bind(manco_entity_id)
                .execute(pool)
                .await?;
            }

            // Auto-tag based on GLEIF category
            let roles_to_assign: Vec<&str> = match category_str.as_deref() {
                Some("FUND") => vec!["FUND"],
                Some("BRANCH") => vec!["BRANCH"],
                Some("SOLE_PROPRIETOR") => vec![], // No role
                Some("GENERAL") | None => {
                    // Check if this is the root (ultimate parent)
                    if lei == &root_lei {
                        vec!["ULTIMATE_PARENT"]
                    } else {
                        vec!["SUBSIDIARY"]
                    }
                }
                _ => vec![],
            };

            for role_name in roles_to_assign {
                if let Some(role_id) = role_map.get(role_name) {
                    let inserted = sqlx::query!(
                        r#"
                        INSERT INTO "ob-poc".client_group_entity_roles
                            (cge_id, role_id, assigned_by, source_record_id)
                        VALUES ($1, $2, 'gleif', $3)
                        ON CONFLICT (cge_id, role_id, COALESCE(target_entity_id, '00000000-0000-0000-0000-000000000000'))
                        DO NOTHING
                        "#,
                        cge_id,
                        role_id,
                        lei
                    )
                    .execute(pool)
                    .await?
                    .rows_affected();
                    roles_assigned += inserted as i64;
                }
            }

            tracing::debug!(
                entity_id = %entity_id,
                entity_name = %entity_name,
                lei = %lei,
                "Added entity to client group"
            );
        }

        // Now create relationship edges from parent_relationships table
        let parent_rels: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
            r#"
            SELECT
                pr.child_entity_id,
                pr.parent_entity_id,
                pr.relationship_type
            FROM "ob-poc".entity_parent_relationships pr
            JOIN "ob-poc".client_group_entity cge_child
                ON cge_child.entity_id = pr.child_entity_id AND cge_child.group_id = $1
            JOIN "ob-poc".client_group_entity cge_parent
                ON cge_parent.entity_id = pr.parent_entity_id AND cge_parent.group_id = $1
            WHERE pr.relationship_status = 'ACTIVE'
            "#,
        )
        .bind(group_id)
        .fetch_all(pool)
        .await?;

        for (child_id, parent_id, rel_type) in parent_rels {
            // Create client_group_relationship
            let relationship_id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO "ob-poc".client_group_relationship
                    (group_id, parent_entity_id, child_entity_id, relationship_kind)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (group_id, parent_entity_id, child_entity_id, relationship_kind)
                DO UPDATE SET updated_at = NOW()
                RETURNING id
                "#,
            )
            .bind(group_id)
            .bind(parent_id)
            .bind(child_id)
            .bind(if rel_type == "DIRECT_PARENT" {
                "ownership"
            } else {
                "control"
            })
            .fetch_one(pool)
            .await?;

            relationships_created += 1;

            // Add source provenance (GLEIF doesn't provide ownership percentages)
            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".client_group_relationship_sources
                    (relationship_id, source, source_type, confidence_score)
                VALUES ($1, 'gleif', 'discovery', 0.80)
                ON CONFLICT DO NOTHING
                "#,
                relationship_id
            )
            .execute(pool)
            .await?;
        }

        // Update discovery status to complete
        sqlx::query!(
            r#"
            UPDATE "ob-poc".client_group
            SET discovery_status = 'complete',
                discovery_completed_at = NOW(),
                entity_count = (
                    SELECT COUNT(*) FROM "ob-poc".client_group_entity
                    WHERE group_id = $1 AND membership_type != 'historical'
                ),
                updated_at = NOW()
            WHERE id = $1
            "#,
            group_id
        )
        .execute(pool)
        .await?;

        let result = serde_json::json!({
            "group_id": group_id,
            "root_lei": root_lei,
            "include_funds": include_funds,
            "gleif_entities_created": tree_result.entities_created,
            "gleif_entities_updated": tree_result.entities_updated,
            "gleif_relationships_created": tree_result.relationships_created,
            "client_group_entities_added": entities_added,
            "client_group_funds_added": funds_added,
            "fund_metadata_populated": funds_added, // fund_metadata upserted for each fund
            "client_group_roles_assigned": roles_assigned,
            "client_group_relationships_created": relationships_created,
        });

        // Log research action if decision-id provided
        if let Some(decision_id) = extract_uuid_opt(verb_call, _ctx, "decision-id") {
            log_research_action(
                pool,
                decision_id,
                "gleif:import-to-client-group",
                &result,
                entities_added as i32,
                tree_result.entities_updated as i32,
            )
            .await?;
        }

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "Database feature required for gleif.import-to-client-group"
        ))
    }
}

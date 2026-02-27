//! GLEIF API Import for Entity Funds
//!
//! Imports fund data from the GLEIF API using DSL verbs.
//! Uses the gleif.import-managed-funds verb for the actual import.
//!
//! Usage:
//!   # Search by name (legacy mode - direct API)
//!   cargo x gleif-import --search "Allianz Global Investors" --dry-run
//!   cargo x gleif-import --search "Allianz Global Investors" --limit 10
//!
//!   # Fetch by manager LEI using DSL verb (recommended)
//!   cargo x gleif-import --manager-lei OJ2TIQSVQND4IZYYK658 --limit 10
//!   cargo x gleif-import -m OJ2TIQSVQND4IZYYK658 --create-cbus

#![allow(dead_code)] // Struct fields used for JSON deserialization from GLEIF API

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use ob_poc::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext};

const GLEIF_API_BASE: &str = "https://api.gleif.org/api/v1/lei-records";
const PAGE_SIZE: usize = 100;

// Rate limiting settings
const BASE_DELAY_MS: u64 = 500; // Base delay between API calls
const MAX_RETRIES: u32 = 5; // Maximum number of retries on rate limit
const BACKOFF_MULTIPLIER: u64 = 2; // Exponential backoff multiplier

// =============================================================================
// GLEIF API Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct GleifResponse {
    pub data: Vec<GleifRecord>,
    pub meta: GleifMeta,
}

#[derive(Debug, Deserialize)]
pub struct GleifSingleResponse {
    pub data: GleifRecord,
}

#[derive(Debug, Deserialize)]
pub struct GleifMeta {
    pub pagination: GleifPagination,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GleifPagination {
    pub total: usize,
    pub current_page: usize,
    pub last_page: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Fields used for JSON deserialization from GLEIF API
pub struct GleifRecord {
    pub attributes: GleifAttributes,
    #[serde(default)]
    pub relationships: Option<GleifRelationships>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GleifAttributes {
    pub lei: String,
    pub entity: GleifEntity,
    pub registration: GleifRegistration,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GleifEntity {
    pub legal_name: GleifLegalName,
    pub jurisdiction: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub legal_form: Option<GleifLegalForm>,
    pub registered_as: Option<String>,
    pub registered_at: Option<GleifRegisteredAt>,
    pub legal_address: Option<GleifAddress>,
    pub headquarters_address: Option<GleifAddress>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GleifLegalName {
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GleifLegalForm {
    pub id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GleifRegisteredAt {
    pub id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GleifAddress {
    pub city: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GleifRegistration {
    pub corroboration_level: Option<String>,
    pub managing_lou: Option<String>,
    pub last_update_date: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)] // Fields used for JSON deserialization from GLEIF API
pub struct GleifRelationships {
    pub direct_parent: Option<GleifRelationshipLink>,
    pub ultimate_parent: Option<GleifRelationshipLink>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Fields used for JSON deserialization from GLEIF API
pub struct GleifRelationshipLink {
    pub links: Option<GleifLinks>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)] // Fields used for JSON deserialization from GLEIF API
pub struct GleifLinks {
    pub related: Option<String>,
}

// Parent relationship response
#[derive(Debug, Deserialize)]
pub struct GleifParentResponse {
    pub data: Option<GleifParentData>,
}

#[derive(Debug, Deserialize)]
pub struct GleifParentData {
    pub attributes: Option<GleifParentAttributes>,
}

#[derive(Debug, Deserialize)]
pub struct GleifParentAttributes {
    pub relationship: Option<GleifParentRelationship>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GleifParentRelationship {
    pub start_node: Option<GleifNode>,
    pub end_node: Option<GleifNode>,
}

#[derive(Debug, Deserialize)]
pub struct GleifNode {
    pub id: Option<String>,
}

// =============================================================================
// Import Statistics
// =============================================================================

#[derive(Debug, Default)]
pub struct ImportStats {
    pub fetched: usize,
    pub upserted: usize,
    pub parents_discovered: usize,
    pub relationships_created: usize,
    pub cbus_linked: usize,
    pub cbus_created: usize,
    pub roles_assigned: usize,
    pub errors: usize,
}

// =============================================================================
// DSL-Based Import Function
// =============================================================================

/// Import managed funds using the gleif.import-managed-funds DSL verb
///
/// This function builds a DSL command and executes it through the standard
/// DSL pipeline (parse → compile → execute), ensuring the verb handlers
/// are properly tested.
async fn gleif_import_via_dsl(
    manager_lei: &str,
    limit: Option<usize>,
    dry_run: bool,
    create_cbus: bool,
) -> Result<()> {
    println!("Using DSL verb: gleif.import-managed-funds\n");

    // Connect to database
    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    // Build the DSL command
    let mut dsl = format!(
        r#"(gleif.import-managed-funds :manager-lei "{}" :create-cbus {}"#,
        manager_lei, create_cbus
    );

    if let Some(lim) = limit {
        dsl.push_str(&format!(" :limit {}", lim));
    }

    if dry_run {
        dsl.push_str(" :dry-run true");
    }

    dsl.push(')');

    println!("DSL command:\n  {}\n", dsl);

    // Parse the DSL
    let ast = parse_program(&dsl).map_err(|e| anyhow!("Parse error: {:?}", e))?;
    println!("Parsed {} statement(s)", ast.statements.len());

    // Compile to execution plan
    let plan = compile(&ast).map_err(|e| anyhow!("Compile error: {:?}", e))?;
    println!("Compiled to {} step(s)", plan.steps.len());

    // Execute
    let executor = DslExecutor::new(pool);
    let mut ctx = ExecutionContext::new();

    println!("\nExecuting...\n");
    let start = std::time::Instant::now();

    let results = executor
        .execute_plan(&plan, &mut ctx)
        .await
        .map_err(|e| anyhow!("Execution error: {:?}", e))?;

    let elapsed = start.elapsed();

    // Print results
    println!("===========================================");
    println!("  Import Complete ({:.2}s)", elapsed.as_secs_f64());
    println!("===========================================\n");

    for (i, result) in results.iter().enumerate() {
        println!("Step {}: {:?}", i + 1, result);
    }

    // Print bindings
    if !ctx.symbols.is_empty() {
        println!("\nBindings:");
        for (name, id) in &ctx.symbols {
            println!("  @{} = {}", name, id);
        }
    }

    Ok(())
}

// =============================================================================
// Main Import Function (Legacy for search mode)
// =============================================================================

pub async fn gleif_import(
    search_term: Option<&str>,
    manager_lei: Option<&str>,
    limit: Option<usize>,
    dry_run: bool,
    create_cbus: bool,
) -> Result<()> {
    println!("===========================================");
    println!("  GLEIF Fund Import via DSL Verbs");
    println!("===========================================\n");

    if let Some(search) = search_term {
        println!("Search term: {}", search);
    }
    if let Some(lei) = manager_lei {
        println!("Manager LEI: {}", lei);
    }
    println!("Limit: {:?}", limit);
    println!("Dry run: {}", dry_run);
    println!("Create CBUs: {}", create_cbus);
    println!();

    // Use DSL verb for manager-lei imports
    if let Some(lei) = manager_lei {
        return gleif_import_via_dsl(lei, limit, dry_run, create_cbus).await;
    }

    // Legacy mode: search by name (direct API, not via DSL)
    println!("Note: Search mode uses legacy direct API. Use --manager-lei for DSL-based import.\n");

    let client = reqwest::Client::new();

    // Fetch records based on mode
    let records = if let Some(lei) = manager_lei {
        fetch_managed_funds(&client, lei, limit).await?
    } else if let Some(search) = search_term {
        fetch_all_records(&client, search, limit).await?
    } else {
        anyhow::bail!("Either --search or --manager-lei must be provided");
    };
    println!("\nFetched {} records from GLEIF API\n", records.len());

    if records.is_empty() {
        println!("No records found.");
        return Ok(());
    }

    if dry_run {
        print_dry_run_summary(&records);
        return Ok(());
    }

    // Connect to database
    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    // Get entity type IDs
    let fund_type_id = get_or_create_entity_type(&pool, "fund_subfund", "Fund (Sub-fund)").await?;
    let company_type_id =
        get_or_create_entity_type(&pool, "limited_company", "Limited Company").await?;

    let mut stats = ImportStats {
        fetched: records.len(),
        ..Default::default()
    };

    // Track all LEIs we've processed to avoid duplicates
    let mut processed_leis: HashSet<String> = HashSet::new();
    // Map LEI -> entity_id for relationship creation
    let mut lei_to_entity: HashMap<String, Uuid> = HashMap::new();
    // Track parent relationships to create
    let mut parent_relationships: Vec<(String, String)> = Vec::new(); // (child_lei, parent_lei)

    // Phase 1: Upsert all fund records
    println!("Phase 1: Upserting fund entities...");
    for (i, record) in records.iter().enumerate() {
        let lei = &record.attributes.lei;

        if processed_leis.contains(lei) {
            continue;
        }
        processed_leis.insert(lei.clone());

        if (i + 1) % 50 == 0 || i == 0 {
            println!(
                "[{}/{}] Processing {}...",
                i + 1,
                records.len(),
                &record.attributes.entity.legal_name.name
            );
        }

        match upsert_entity(&pool, record, fund_type_id).await {
            Ok(entity_id) => {
                stats.upserted += 1;
                lei_to_entity.insert(lei.clone(), entity_id);
            }
            Err(e) => {
                eprintln!("  Error upserting {}: {}", lei, e);
                stats.errors += 1;
            }
        }
    }

    // Phase 2: Walk up ownership chain for each entity
    println!("\nPhase 2: Walking ownership chains...");
    let leis_to_check: Vec<String> = lei_to_entity.keys().cloned().collect();

    for lei in &leis_to_check {
        match walk_parent_chain(
            &client,
            &pool,
            lei,
            &mut processed_leis,
            &mut lei_to_entity,
            &mut parent_relationships,
            company_type_id,
            &mut stats,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                eprintln!("  Error walking chain for {}: {}", lei, e);
            }
        }
        // Rate limiting - more conservative for parent chain walking
        tokio::time::sleep(tokio::time::Duration::from_millis(BASE_DELAY_MS)).await;
    }

    // Phase 3: Create entity_relationships
    println!("\nPhase 3: Creating ownership relationships...");
    for (child_lei, parent_lei) in &parent_relationships {
        if let (Some(&child_id), Some(&parent_id)) =
            (lei_to_entity.get(child_lei), lei_to_entity.get(parent_lei))
        {
            match create_ownership_relationship(&pool, parent_id, child_id).await {
                Ok(created) => {
                    if created {
                        stats.relationships_created += 1;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "  Error creating relationship {} -> {}: {}",
                        parent_lei, child_lei, e
                    );
                }
            }
        }
    }

    // Phase 4: Create CBUs if requested
    if create_cbus {
        println!("\nPhase 4: Creating CBUs for funds...");

        // Get the manager entity if we have a manager_lei
        let manager_entity_id = if let Some(lei) = manager_lei {
            get_entity_by_lei(&pool, lei).await?
        } else {
            None
        };

        for (lei, &entity_id) in &lei_to_entity {
            // Only create CBUs for FUND category entities
            let is_fund: bool = sqlx::query_scalar(
                r#"SELECT COALESCE(gleif_category = 'FUND', false) FROM "ob-poc".entity_funds WHERE entity_id = $1"#
            )
            .bind(entity_id)
            .fetch_optional(&pool)
            .await?
            .unwrap_or(false);

            if !is_fund {
                continue;
            }

            match create_cbu_for_fund(&pool, entity_id, manager_entity_id).await {
                Ok((cbu_created, roles_assigned)) => {
                    if cbu_created {
                        stats.cbus_created += 1;
                    }
                    stats.roles_assigned += roles_assigned;
                }
                Err(e) => {
                    eprintln!("  Error creating CBU for {}: {}", lei, e);
                    stats.errors += 1;
                }
            }
        }
    } else if let Some(search) = search_term {
        // Legacy behavior: link CBUs to head office
        println!("\nPhase 4: Linking CBUs to head office...");
        if let Some(head_office_id) = find_head_office(&pool, search).await? {
            stats.cbus_linked = link_cbus_to_head_office(&pool, search, head_office_id).await?;
        }
    }

    // Print summary
    print_summary(&stats, create_cbus);

    Ok(())
}

// =============================================================================
// API Fetching with Rate Limiting
// =============================================================================

/// Makes a GET request with exponential backoff on 429 rate limit errors
async fn fetch_with_retry<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    url: &str,
) -> Result<Option<T>> {
    let mut delay_ms = BASE_DELAY_MS;

    for attempt in 0..MAX_RETRIES {
        let response = client
            .get(url)
            .header("Accept", "application/vnd.api+json")
            .send()
            .await
            .context("GLEIF API request failed")?;

        let status = response.status();

        // Handle rate limiting (429)
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            if attempt < MAX_RETRIES - 1 {
                println!(
                    "    Rate limited (429), waiting {}ms before retry {}/{}...",
                    delay_ms,
                    attempt + 1,
                    MAX_RETRIES
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                delay_ms *= BACKOFF_MULTIPLIER;
                continue;
            } else {
                return Err(anyhow!(
                    "Rate limited by GLEIF API after {} retries",
                    MAX_RETRIES
                ));
            }
        }

        // Handle not found
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        // Handle other errors
        if !status.is_success() {
            return Err(anyhow!("GLEIF API returned status {}", status));
        }

        // Parse response
        let data: T = response
            .json()
            .await
            .context("Failed to parse GLEIF response")?;
        return Ok(Some(data));
    }

    Err(anyhow!("Exceeded maximum retries"))
}

async fn fetch_all_records(
    client: &reqwest::Client,
    search_term: &str,
    limit: Option<usize>,
) -> Result<Vec<GleifRecord>> {
    let mut all_records = Vec::new();
    let mut page = 1;

    loop {
        println!("Fetching page {}...", page);

        let url = format!(
            "{}?filter%5Bentity.names%5D={}&page%5Bnumber%5D={}&page%5Bsize%5D={}",
            GLEIF_API_BASE,
            urlencoding::encode(search_term),
            page,
            PAGE_SIZE
        );

        let response: GleifResponse = fetch_with_retry(client, &url)
            .await?
            .ok_or_else(|| anyhow!("Unexpected 404 for search results"))?;

        println!(
            "  Got {} records (page {}/{}, total: {})",
            response.data.len(),
            response.meta.pagination.current_page,
            response.meta.pagination.last_page,
            response.meta.pagination.total
        );

        all_records.extend(response.data);

        if let Some(max) = limit {
            if all_records.len() >= max {
                all_records.truncate(max);
                break;
            }
        }

        if page >= response.meta.pagination.last_page {
            break;
        }

        page += 1;
        // Rate limiting delay between pages
        tokio::time::sleep(tokio::time::Duration::from_millis(BASE_DELAY_MS)).await;
    }

    Ok(all_records)
}

/// Fetch all funds managed by a given fund manager LEI
/// Uses the GLEIF relationship endpoint: /{manager_lei}/managed-funds
async fn fetch_managed_funds(
    client: &reqwest::Client,
    manager_lei: &str,
    limit: Option<usize>,
) -> Result<Vec<GleifRecord>> {
    let mut all_records = Vec::new();
    let mut page = 1;

    println!("Fetching funds managed by LEI: {}", manager_lei);

    loop {
        println!("Fetching page {}...", page);

        // Use the managed-funds relationship endpoint
        let url = format!(
            "{}/{}/managed-funds?page%5Bnumber%5D={}&page%5Bsize%5D={}",
            GLEIF_API_BASE, manager_lei, page, PAGE_SIZE
        );

        let response: GleifResponse = fetch_with_retry(client, &url).await?.ok_or_else(|| {
            anyhow!(
                "Unexpected 404 for managed funds query - is {} a valid fund manager LEI?",
                manager_lei
            )
        })?;

        println!(
            "  Got {} records (page {}/{}, total: {})",
            response.data.len(),
            response.meta.pagination.current_page,
            response.meta.pagination.last_page,
            response.meta.pagination.total
        );

        all_records.extend(response.data);

        if let Some(max) = limit {
            if all_records.len() >= max {
                all_records.truncate(max);
                break;
            }
        }

        if page >= response.meta.pagination.last_page {
            break;
        }

        page += 1;
        // Rate limiting delay between pages
        tokio::time::sleep(tokio::time::Duration::from_millis(BASE_DELAY_MS)).await;
    }

    Ok(all_records)
}

async fn fetch_single_lei(client: &reqwest::Client, lei: &str) -> Result<Option<GleifRecord>> {
    let url = format!("{}/{}", GLEIF_API_BASE, lei);
    let response: Option<GleifSingleResponse> = fetch_with_retry(client, &url).await?;
    Ok(response.map(|r| r.data))
}

async fn fetch_direct_parent_lei(client: &reqwest::Client, lei: &str) -> Result<Option<String>> {
    let url = format!("{}/{}/direct-parent-relationship", GLEIF_API_BASE, lei);
    let response: Option<GleifParentResponse> = fetch_with_retry(client, &url).await?;

    // Extract parent LEI from relationship
    // The relationship is "IS_DIRECTLY_CONSOLIDATED_BY" where:
    // - start_node = the child entity (the one we queried)
    // - end_node = the parent entity (what we want)
    let parent_lei = response
        .and_then(|r| r.data)
        .and_then(|d| d.attributes)
        .and_then(|a| a.relationship)
        .and_then(|r| r.end_node)
        .and_then(|n| n.id);

    Ok(parent_lei)
}

// =============================================================================
// Parent Chain Walking
// =============================================================================

#[allow(clippy::too_many_arguments)]
async fn walk_parent_chain(
    client: &reqwest::Client,
    pool: &PgPool,
    start_lei: &str,
    processed_leis: &mut HashSet<String>,
    lei_to_entity: &mut HashMap<String, Uuid>,
    parent_relationships: &mut Vec<(String, String)>,
    company_type_id: Uuid,
    stats: &mut ImportStats,
) -> Result<()> {
    let mut current_lei = start_lei.to_string();
    let mut depth = 0;
    const MAX_DEPTH: usize = 10; // Prevent infinite loops

    while depth < MAX_DEPTH {
        // Fetch parent LEI
        let parent_lei = match fetch_direct_parent_lei(client, &current_lei).await? {
            Some(lei) => lei,
            None => break, // No more parents
        };

        // Skip if we've already processed this parent
        if processed_leis.contains(&parent_lei) {
            // Still record the relationship
            parent_relationships.push((current_lei.clone(), parent_lei.clone()));
            break;
        }

        println!("  Discovered parent: {} (depth {})", parent_lei, depth + 1);
        stats.parents_discovered += 1;

        // Fetch and upsert parent entity
        if let Some(parent_record) = fetch_single_lei(client, &parent_lei).await? {
            processed_leis.insert(parent_lei.clone());

            // Determine entity type based on GLEIF category
            let entity_type_id =
                if parent_record.attributes.entity.category.as_deref() == Some("FUND") {
                    get_or_create_entity_type(pool, "fund_subfund", "Fund (Sub-fund)").await?
                } else {
                    company_type_id
                };

            match upsert_entity(pool, &parent_record, entity_type_id).await {
                Ok(entity_id) => {
                    stats.upserted += 1;
                    lei_to_entity.insert(parent_lei.clone(), entity_id);
                }
                Err(e) => {
                    eprintln!("    Error upserting parent {}: {}", parent_lei, e);
                    stats.errors += 1;
                }
            }
        }

        // Record the relationship
        parent_relationships.push((current_lei.clone(), parent_lei.clone()));

        current_lei = parent_lei;
        depth += 1;

        // Rate limiting between parent lookups
        tokio::time::sleep(tokio::time::Duration::from_millis(BASE_DELAY_MS)).await;
    }

    Ok(())
}

// =============================================================================
// Database Operations
// =============================================================================

async fn upsert_entity(pool: &PgPool, record: &GleifRecord, entity_type_id: Uuid) -> Result<Uuid> {
    let attrs = &record.attributes;
    let entity = &attrs.entity;
    let registration = &attrs.registration;
    let name = &entity.legal_name.name;
    let lei = &attrs.lei;

    let last_update: Option<DateTime<Utc>> = registration
        .last_update_date
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    // Check if entity with this LEI already exists
    let existing: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT entity_id FROM "ob-poc".entity_funds WHERE lei = $1"#)
            .bind(lei)
            .fetch_optional(pool)
            .await?;

    if let Some((entity_id,)) = existing {
        // Update existing record
        sqlx::query(
            r#"
            UPDATE "ob-poc".entity_funds SET
                gleif_category = $2,
                gleif_status = $3,
                gleif_legal_form_id = $4,
                gleif_registered_as = $5,
                gleif_registered_at = $6,
                gleif_corroboration_level = $7,
                gleif_managing_lou = $8,
                gleif_last_update = $9,
                legal_address_city = $10,
                legal_address_country = $11,
                hq_address_city = $12,
                hq_address_country = $13,
                jurisdiction = COALESCE(jurisdiction, $14),
                updated_at = now()
            WHERE lei = $1
            "#,
        )
        .bind(lei)
        .bind(&entity.category)
        .bind(&entity.status)
        .bind(entity.legal_form.as_ref().and_then(|lf| lf.id.as_ref()))
        .bind(&entity.registered_as)
        .bind(entity.registered_at.as_ref().and_then(|ra| ra.id.as_ref()))
        .bind(&registration.corroboration_level)
        .bind(&registration.managing_lou)
        .bind(last_update)
        .bind(entity.legal_address.as_ref().and_then(|a| a.city.as_ref()))
        .bind(
            entity
                .legal_address
                .as_ref()
                .and_then(|a| a.country.as_ref()),
        )
        .bind(
            entity
                .headquarters_address
                .as_ref()
                .and_then(|a| a.city.as_ref()),
        )
        .bind(
            entity
                .headquarters_address
                .as_ref()
                .and_then(|a| a.country.as_ref()),
        )
        .bind(&entity.jurisdiction)
        .execute(pool)
        .await?;

        // Also update the entity name if changed
        sqlx::query(
            r#"UPDATE "ob-poc".entities SET name = $2, updated_at = now() WHERE entity_id = $1"#,
        )
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await?;

        return Ok(entity_id);
    }

    // Check if entity exists by name (without LEI) - match and add LEI
    let existing_by_name: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT e.entity_id
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
        WHERE UPPER(e.name) = UPPER($1) AND ef.lei IS NULL
        LIMIT 1
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;

    if let Some((entity_id,)) = existing_by_name {
        // Update existing entity with LEI
        sqlx::query(
            r#"
            UPDATE "ob-poc".entity_funds SET
                lei = $2,
                gleif_category = $3,
                gleif_status = $4,
                gleif_legal_form_id = $5,
                gleif_registered_as = $6,
                gleif_registered_at = $7,
                gleif_corroboration_level = $8,
                gleif_managing_lou = $9,
                gleif_last_update = $10,
                legal_address_city = $11,
                legal_address_country = $12,
                hq_address_city = $13,
                hq_address_country = $14,
                jurisdiction = COALESCE(jurisdiction, $15),
                updated_at = now()
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .bind(lei)
        .bind(&entity.category)
        .bind(&entity.status)
        .bind(entity.legal_form.as_ref().and_then(|lf| lf.id.as_ref()))
        .bind(&entity.registered_as)
        .bind(entity.registered_at.as_ref().and_then(|ra| ra.id.as_ref()))
        .bind(&registration.corroboration_level)
        .bind(&registration.managing_lou)
        .bind(last_update)
        .bind(entity.legal_address.as_ref().and_then(|a| a.city.as_ref()))
        .bind(
            entity
                .legal_address
                .as_ref()
                .and_then(|a| a.country.as_ref()),
        )
        .bind(
            entity
                .headquarters_address
                .as_ref()
                .and_then(|a| a.city.as_ref()),
        )
        .bind(
            entity
                .headquarters_address
                .as_ref()
                .and_then(|a| a.country.as_ref()),
        )
        .bind(&entity.jurisdiction)
        .execute(pool)
        .await?;

        return Ok(entity_id);
    }

    // Check if entity exists by name (any type) - if so, use that entity
    let existing_any: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT e.entity_id
        FROM "ob-poc".entities e
        WHERE UPPER(e.name) = UPPER($1)
        LIMIT 1
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await?;

    if let Some((entity_id,)) = existing_any {
        // Entity exists but doesn't have entity_funds record - create one
        let has_funds: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entity_funds WHERE entity_id = $1)"#,
        )
        .bind(entity_id)
        .fetch_one(pool)
        .await?;

        if !has_funds {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".entity_funds (
                    entity_id, lei, jurisdiction,
                    gleif_category, gleif_status, gleif_legal_form_id,
                    gleif_registered_as, gleif_registered_at,
                    gleif_corroboration_level, gleif_managing_lou, gleif_last_update,
                    legal_address_city, legal_address_country,
                    hq_address_city, hq_address_country
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                "#,
            )
            .bind(entity_id)
            .bind(lei)
            .bind(&entity.jurisdiction)
            .bind(&entity.category)
            .bind(&entity.status)
            .bind(entity.legal_form.as_ref().and_then(|lf| lf.id.as_ref()))
            .bind(&entity.registered_as)
            .bind(entity.registered_at.as_ref().and_then(|ra| ra.id.as_ref()))
            .bind(&registration.corroboration_level)
            .bind(&registration.managing_lou)
            .bind(last_update)
            .bind(entity.legal_address.as_ref().and_then(|a| a.city.as_ref()))
            .bind(
                entity
                    .legal_address
                    .as_ref()
                    .and_then(|a| a.country.as_ref()),
            )
            .bind(
                entity
                    .headquarters_address
                    .as_ref()
                    .and_then(|a| a.city.as_ref()),
            )
            .bind(
                entity
                    .headquarters_address
                    .as_ref()
                    .and_then(|a| a.country.as_ref()),
            )
            .execute(pool)
            .await?;
        } else {
            // Update existing entity_funds with LEI
            sqlx::query(
                r#"
                UPDATE "ob-poc".entity_funds SET
                    lei = COALESCE(lei, $2),
                    gleif_category = $3,
                    gleif_status = $4,
                    gleif_legal_form_id = $5,
                    gleif_registered_as = $6,
                    gleif_registered_at = $7,
                    gleif_corroboration_level = $8,
                    gleif_managing_lou = $9,
                    gleif_last_update = $10,
                    legal_address_city = $11,
                    legal_address_country = $12,
                    hq_address_city = $13,
                    hq_address_country = $14,
                    jurisdiction = COALESCE(jurisdiction, $15),
                    updated_at = now()
                WHERE entity_id = $1
                "#,
            )
            .bind(entity_id)
            .bind(lei)
            .bind(&entity.category)
            .bind(&entity.status)
            .bind(entity.legal_form.as_ref().and_then(|lf| lf.id.as_ref()))
            .bind(&entity.registered_as)
            .bind(entity.registered_at.as_ref().and_then(|ra| ra.id.as_ref()))
            .bind(&registration.corroboration_level)
            .bind(&registration.managing_lou)
            .bind(last_update)
            .bind(entity.legal_address.as_ref().and_then(|a| a.city.as_ref()))
            .bind(
                entity
                    .legal_address
                    .as_ref()
                    .and_then(|a| a.country.as_ref()),
            )
            .bind(
                entity
                    .headquarters_address
                    .as_ref()
                    .and_then(|a| a.city.as_ref()),
            )
            .bind(
                entity
                    .headquarters_address
                    .as_ref()
                    .and_then(|a| a.country.as_ref()),
            )
            .bind(&entity.jurisdiction)
            .execute(pool)
            .await?;
        }
        return Ok(entity_id);
    }

    // Create new entity
    let entity_id = Uuid::new_v4();
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name) VALUES ($1, $2, $3)"#,
    )
    .bind(entity_id)
    .bind(entity_type_id)
    .bind(name)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".entity_funds (
            entity_id, lei, jurisdiction,
            gleif_category, gleif_status, gleif_legal_form_id,
            gleif_registered_as, gleif_registered_at,
            gleif_corroboration_level, gleif_managing_lou, gleif_last_update,
            legal_address_city, legal_address_country,
            hq_address_city, hq_address_country
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#,
    )
    .bind(entity_id)
    .bind(lei)
    .bind(&entity.jurisdiction)
    .bind(&entity.category)
    .bind(&entity.status)
    .bind(entity.legal_form.as_ref().and_then(|lf| lf.id.as_ref()))
    .bind(&entity.registered_as)
    .bind(entity.registered_at.as_ref().and_then(|ra| ra.id.as_ref()))
    .bind(&registration.corroboration_level)
    .bind(&registration.managing_lou)
    .bind(last_update)
    .bind(entity.legal_address.as_ref().and_then(|a| a.city.as_ref()))
    .bind(
        entity
            .legal_address
            .as_ref()
            .and_then(|a| a.country.as_ref()),
    )
    .bind(
        entity
            .headquarters_address
            .as_ref()
            .and_then(|a| a.city.as_ref()),
    )
    .bind(
        entity
            .headquarters_address
            .as_ref()
            .and_then(|a| a.country.as_ref()),
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(entity_id)
}

async fn create_ownership_relationship(
    pool: &PgPool,
    owner_id: Uuid,
    owned_id: Uuid,
) -> Result<bool> {
    // Check if relationship already exists
    let existing: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT relationship_id FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1 AND to_entity_id = $2 AND relationship_type = 'ownership'
        "#,
    )
    .bind(owner_id)
    .bind(owned_id)
    .fetch_optional(pool)
    .await?;

    let relationship_id = if let Some((id,)) = existing {
        id
    } else {
        // GLEIF direct-parent-relationship means 100% consolidation (or at least majority control)
        // Use 100.00 as the percentage for direct consolidation relationships
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_relationships (
                relationship_id, from_entity_id, to_entity_id, relationship_type, percentage, source, effective_from
            ) VALUES ($1, $2, $3, 'ownership', 100.00, 'GLEIF', CURRENT_DATE)
            "#,
        )
        .bind(id)
        .bind(owner_id)
        .bind(owned_id)
        .execute(pool)
        .await?;
        id
    };

    // Create cbu_relationship_verification records for all CBUs that have the owned entity
    // This links the structural relationship to specific CBU contexts
    let cbu_ids: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT c.cbu_id
        FROM "ob-poc".cbus c
        JOIN "ob-poc".cbu_entity_roles cer ON c.cbu_id = cer.cbu_id
        WHERE cer.entity_id = $1 OR cer.entity_id = $2
        "#,
    )
    .bind(owner_id)
    .bind(owned_id)
    .fetch_all(pool)
    .await?;

    for (cbu_id,) in cbu_ids {
        // Check if verification record already exists
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".cbu_relationship_verification
                WHERE cbu_id = $1 AND relationship_id = $2
            )
            "#,
        )
        .bind(cbu_id)
        .bind(relationship_id)
        .fetch_one(pool)
        .await?;

        if !exists {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".cbu_relationship_verification (
                    cbu_id, relationship_id, status, observed_percentage
                ) VALUES ($1, $2, 'proven', 100.00)
                "#,
            )
            .bind(cbu_id)
            .bind(relationship_id)
            .execute(pool)
            .await?;
        }
    }

    Ok(existing.is_none())
}

async fn find_head_office(pool: &PgPool, search_term: &str) -> Result<Option<Uuid>> {
    // For Allianz Global Investors, the head office is the GmbH (management company)
    // Look for "GmbH" in the name with category GENERAL
    let result: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT e.entity_id
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
        WHERE e.name ILIKE $1
          AND e.name ILIKE '%GmbH%'
          AND ef.gleif_category = 'GENERAL'
        ORDER BY e.name
        LIMIT 1
        "#,
    )
    .bind(format!("%{}%", search_term))
    .fetch_optional(pool)
    .await?;

    if let Some((id,)) = result {
        let name: String =
            sqlx::query_scalar(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                .bind(id)
                .fetch_one(pool)
                .await?;
        println!("  Found head office (GmbH): {} ({})", name, id);
        return Ok(Some(id));
    }

    // Fallback: look for any GENERAL category entity matching the search
    let result: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT e.entity_id
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
        WHERE e.name ILIKE $1
          AND ef.gleif_category = 'GENERAL'
        ORDER BY e.name
        LIMIT 1
        "#,
    )
    .bind(format!("%{}%", search_term))
    .fetch_optional(pool)
    .await?;

    if let Some((id,)) = result {
        let name: String =
            sqlx::query_scalar(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                .bind(id)
                .fetch_one(pool)
                .await?;
        println!("  Found head office (GENERAL): {} ({})", name, id);
        return Ok(Some(id));
    }

    // Last resort: find any entity that is a parent of funds
    let result: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT er.from_entity_id as entity_id
        FROM "ob-poc".entity_relationships er
        JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
        WHERE er.relationship_type = 'ownership'
          AND e.name ILIKE $1
        LIMIT 1
        "#,
    )
    .bind(format!("%{}%", search_term))
    .fetch_optional(pool)
    .await?;

    if let Some((id,)) = result {
        let name: String =
            sqlx::query_scalar(r#"SELECT name FROM "ob-poc".entities WHERE entity_id = $1"#)
                .bind(id)
                .fetch_one(pool)
                .await?;
        println!("  Found head office (parent): {} ({})", name, id);
        return Ok(Some(id));
    }

    println!("  No head office found");
    Ok(None)
}

async fn link_cbus_to_head_office(
    pool: &PgPool,
    search_term: &str,
    head_office_id: Uuid,
) -> Result<usize> {
    // Extract the first word of the search term for broader matching
    // e.g., "Allianz Global Investors" -> "Allianz"
    let first_word = search_term.split_whitespace().next().unwrap_or(search_term);

    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".cbus
        SET commercial_client_entity_id = $1, updated_at = now()
        WHERE name ILIKE $2
          AND commercial_client_entity_id IS NULL
        "#,
    )
    .bind(head_office_id)
    .bind(format!("{}%", first_word))
    .execute(pool)
    .await?;

    let count = result.rows_affected() as usize;
    println!(
        "  Linked {} CBUs to head office (matching '{}')",
        count, first_word
    );
    Ok(count)
}

async fn get_or_create_entity_type(pool: &PgPool, type_code: &str, name: &str) -> Result<Uuid> {
    // Try to get existing
    let existing: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = $1"#)
            .bind(type_code)
            .fetch_optional(pool)
            .await?;

    if let Some((id,)) = existing {
        return Ok(id);
    }

    // Create new
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".entity_types (entity_type_id, type_code, name, table_name) VALUES ($1, $2, $3, $4)"#
    )
    .bind(id)
    .bind(type_code)
    .bind(name)
    .bind(format!("entity_{}", type_code.replace('-', "_")))
    .execute(pool)
    .await?;

    Ok(id)
}

// =============================================================================
// CBU Creation Helpers
// =============================================================================

/// Get entity by LEI
async fn get_entity_by_lei(pool: &PgPool, lei: &str) -> Result<Option<Uuid>> {
    let result: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT entity_id FROM "ob-poc".entity_funds WHERE lei = $1"#)
            .bind(lei)
            .fetch_optional(pool)
            .await?;
    Ok(result.map(|(id,)| id))
}

/// Create a CBU for a fund entity, with ASSET_OWNER role and optionally INVESTMENT_MANAGER role
/// Returns (cbu_created, roles_assigned)
async fn create_cbu_for_fund(
    pool: &PgPool,
    fund_entity_id: Uuid,
    manager_entity_id: Option<Uuid>,
) -> Result<(bool, usize)> {
    // Get fund name and jurisdiction
    let fund_info: Option<(String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT e.name, ef.jurisdiction
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_funds ef ON e.entity_id = ef.entity_id
        WHERE e.entity_id = $1
        "#,
    )
    .bind(fund_entity_id)
    .fetch_optional(pool)
    .await?;

    let (fund_name, jurisdiction) = match fund_info {
        Some(info) => info,
        None => return Err(anyhow!("Fund entity not found: {}", fund_entity_id)),
    };

    // Check if CBU already exists for this fund name
    let existing_cbu: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1"#)
            .bind(&fund_name)
            .fetch_optional(pool)
            .await?;

    let (cbu_id, cbu_created) = if let Some((id,)) = existing_cbu {
        (id, false)
    } else {
        // Create new CBU
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type)
            VALUES ($1, $2, $3, 'fund')
            "#,
        )
        .bind(id)
        .bind(&fund_name)
        .bind(&jurisdiction)
        .execute(pool)
        .await?;
        println!("  Created CBU: {}", fund_name);
        (id, true)
    };

    let mut roles_assigned = 0;

    // Get or create ASSET_OWNER role
    let asset_owner_role_id = get_or_create_role(pool, "ASSET_OWNER").await?;

    // Assign ASSET_OWNER role (fund owns itself)
    if assign_role_if_not_exists(pool, cbu_id, fund_entity_id, asset_owner_role_id).await? {
        roles_assigned += 1;
    }

    // Assign INVESTMENT_MANAGER role if we have a manager
    if let Some(manager_id) = manager_entity_id {
        let im_role_id = get_or_create_role(pool, "INVESTMENT_MANAGER").await?;
        if assign_role_if_not_exists(pool, cbu_id, manager_id, im_role_id).await? {
            roles_assigned += 1;
        }
    }

    Ok((cbu_created, roles_assigned))
}

/// Get or create a role by name
async fn get_or_create_role(pool: &PgPool, role_name: &str) -> Result<Uuid> {
    let existing: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT role_id FROM "ob-poc".roles WHERE name = $1"#)
            .bind(role_name)
            .fetch_optional(pool)
            .await?;

    if let Some((id,)) = existing {
        return Ok(id);
    }

    let id = Uuid::new_v4();
    sqlx::query(r#"INSERT INTO "ob-poc".roles (role_id, name) VALUES ($1, $2)"#)
        .bind(id)
        .bind(role_name)
        .execute(pool)
        .await?;
    Ok(id)
}

/// Assign role if it doesn't already exist, returns true if created
async fn assign_role_if_not_exists(
    pool: &PgPool,
    cbu_id: Uuid,
    entity_id: Uuid,
    role_id: Uuid,
) -> Result<bool> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM "ob-poc".cbu_entity_roles
            WHERE cbu_id = $1 AND entity_id = $2 AND role_id = $3
        )
        "#,
    )
    .bind(cbu_id)
    .bind(entity_id)
    .bind(role_id)
    .fetch_one(pool)
    .await?;

    if exists {
        return Ok(false);
    }

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(cbu_id)
    .bind(entity_id)
    .bind(role_id)
    .execute(pool)
    .await?;

    Ok(true)
}

// =============================================================================
// Output Helpers
// =============================================================================

fn print_dry_run_summary(records: &[GleifRecord]) {
    println!("DRY RUN - would import:");
    for (i, record) in records.iter().enumerate().take(20) {
        let name = &record.attributes.entity.legal_name.name;
        let lei = &record.attributes.lei;
        let category = record.attributes.entity.category.as_deref().unwrap_or("?");
        let status = record.attributes.entity.status.as_deref().unwrap_or("?");
        println!(
            "  [{:3}] {} ({}) - {} [{}]",
            i + 1,
            lei,
            category,
            name,
            status
        );
    }
    if records.len() > 20 {
        println!("  ... and {} more records", records.len() - 20);
    }
    println!("\nRun without --dry-run to actually import.");
}

fn print_summary(stats: &ImportStats, create_cbus: bool) {
    println!("\n===========================================");
    println!("  Import Summary");
    println!("===========================================");
    println!("Fetched from API:       {}", stats.fetched);
    println!("Entities upserted:      {}", stats.upserted);
    println!("Parents discovered:     {}", stats.parents_discovered);
    println!("Relationships created:  {}", stats.relationships_created);
    if create_cbus {
        println!("CBUs created:           {}", stats.cbus_created);
        println!("Roles assigned:         {}", stats.roles_assigned);
    } else {
        println!("CBUs linked:            {}", stats.cbus_linked);
    }
    println!("Errors:                 {}", stats.errors);
}

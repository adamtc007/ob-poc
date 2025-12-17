//! Allianz test data seeding for batch import testing
//!
//! This module provides commands to:
//! 1. Clean existing Allianz entities from the database
//! 2. Seed fund entities from scraped JSON data
//!
//! Usage:
//!   cargo x seed-allianz              # Clean and seed all 205 funds
//!   cargo x seed-allianz --limit 5    # Seed only first 5 funds
//!   cargo x seed-allianz --dry-run    # Show what would be done
//!   cargo x clean-allianz             # Just clean existing data

use anyhow::{Context, Result};
use serde::Deserialize;
use sqlx::PgPool;
use std::path::PathBuf;
use uuid::Uuid;

/// Scraped fund data structure (matches allianz-lu-*.json)
#[derive(Debug, Deserialize)]
struct AllianzData {
    metadata: Metadata,
    manco: MancoData,
    funds: Vec<FundData>,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    jurisdiction: String,
    manco: String,
    #[serde(rename = "totalFunds")]
    total_funds: usize,
}

#[derive(Debug, Deserialize)]
struct MancoData {
    name: String,
    jurisdiction: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    entity_type: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FundData {
    name: String,
    #[serde(rename = "fundId")]
    fund_id: String,
    #[serde(rename = "assetClass")]
    asset_class: Option<String>,
    #[serde(rename = "legalStructure")]
    legal_structure: Option<String>,
    jurisdiction: String,
    #[serde(rename = "primaryIsin")]
    primary_isin: Option<String>,
}

/// Clean all Allianz-related data from the database
pub async fn clean_allianz(dry_run: bool) -> Result<()> {
    println!("===========================================");
    println!("  Allianz Data Cleanup");
    println!("===========================================\n");

    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());

    let pool = PgPool::connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    // Count existing data
    let counts = count_allianz_data(&pool).await?;
    println!("Found Allianz data:");
    println!("  - CBUs: {}", counts.cbus);
    println!("  - Entities: {}", counts.entities);
    println!("  - CBU-Entity Roles: {}", counts.roles);

    if dry_run {
        println!("\n[DRY RUN] Would delete all of the above.");
        return Ok(());
    }

    if counts.cbus == 0 && counts.entities == 0 {
        println!("\nNo Allianz data to clean.");
        return Ok(());
    }

    println!("\nCleaning...");

    // Use the comprehensive cleanup that handles all FK dependencies
    clean_allianz_internal(&pool).await?;

    println!("\nCleanup complete!");
    Ok(())
}

/// Seed Allianz fund entities from scraped JSON
pub async fn seed_allianz(
    file: Option<PathBuf>,
    limit: Option<usize>,
    no_clean: bool,
    dry_run: bool,
) -> Result<()> {
    println!("===========================================");
    println!("  Allianz Data Seeding");
    println!("===========================================\n");

    // Find the JSON file
    let json_path = match file {
        Some(p) => p,
        None => find_allianz_json()?,
    };

    println!("Loading data from: {}", json_path.display());

    // Parse JSON
    let content = std::fs::read_to_string(&json_path)
        .with_context(|| format!("Failed to read {}", json_path.display()))?;

    let data: AllianzData =
        serde_json::from_str(&content).context("Failed to parse Allianz JSON")?;

    println!(
        "Loaded {} funds from {} (ManCo: {})",
        data.metadata.total_funds, data.metadata.jurisdiction, data.metadata.manco
    );

    let funds_to_seed: Vec<_> = match limit {
        Some(n) => data.funds.into_iter().take(n).collect(),
        None => data.funds,
    };

    println!("Will seed {} funds", funds_to_seed.len());

    if dry_run {
        println!("\n[DRY RUN] Would create:");
        println!("  - 1 ManCo entity: {}", data.manco.name);
        for (i, fund) in funds_to_seed.iter().enumerate().take(10) {
            println!(
                "  - Fund {}: {} ({})",
                i + 1,
                fund.name,
                fund.legal_structure.as_deref().unwrap_or("unknown")
            );
        }
        if funds_to_seed.len() > 10 {
            println!("  ... and {} more funds", funds_to_seed.len() - 10);
        }
        return Ok(());
    }

    let db_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());

    let pool = PgPool::connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    // Clean existing data first (unless --no-clean)
    if !no_clean {
        println!("\nStep 1: Cleaning existing Allianz data...");
        clean_allianz_internal(&pool).await?;
    }

    // Get entity type IDs
    println!("\nStep 2: Looking up entity types...");
    let limited_company_type_id = get_entity_type_id(&pool, "LIMITED_COMPANY_PRIVATE").await?;
    let fund_subfund_type_id = get_entity_type_id(&pool, "fund_subfund").await?;

    // Create ManCo entity
    println!("\nStep 3: Creating ManCo entity...");
    let manco_id = create_entity(
        &pool,
        &data.manco.name,
        limited_company_type_id,
        Some(&data.manco.jurisdiction),
    )
    .await?;
    println!("  Created ManCo: {} ({})", data.manco.name, manco_id);

    // Create fund entities
    println!(
        "\nStep 4: Creating {} fund entities...",
        funds_to_seed.len()
    );
    let mut created = 0;
    let mut errors = 0;

    for fund in &funds_to_seed {
        match create_entity(
            &pool,
            &fund.name,
            fund_subfund_type_id,
            Some(&fund.jurisdiction),
        )
        .await
        {
            Ok(id) => {
                created += 1;
                if created <= 5 || created % 50 == 0 {
                    println!(
                        "  [{}/{}] Created: {} ({})",
                        created,
                        funds_to_seed.len(),
                        fund.name,
                        id
                    );
                }
            }
            Err(e) => {
                errors += 1;
                eprintln!("  ERROR creating {}: {}", fund.name, e);
            }
        }
    }

    println!("\n===========================================");
    println!("  Seeding Complete");
    println!("===========================================");
    println!("  ManCo created: 1");
    println!("  Funds created: {}", created);
    if errors > 0 {
        println!("  Errors: {}", errors);
    }

    // Show final counts
    let counts = count_allianz_data(&pool).await?;
    println!("\nDatabase now contains:");
    println!("  - Allianz entities: {}", counts.entities);
    println!(
        "  - Allianz CBUs: {} (will be created by batch import)",
        counts.cbus
    );

    Ok(())
}

/// Find the most recent Allianz JSON file
fn find_allianz_json() -> Result<PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR not set")?;
    let project_root = PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let output_dir = project_root.join("scrapers/allianz/output");

    if !output_dir.exists() {
        anyhow::bail!(
            "Scrapers output directory not found: {}",
            output_dir.display()
        );
    }

    // Find allianz-lu-YYYY-MM-DD.json files (exclude -raw and -detailed variants)
    let mut json_files: Vec<_> = std::fs::read_dir(&output_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("allianz-lu-")
                && name.ends_with(".json")
                && !name.contains("-raw")
                && !name.contains("-detailed")
        })
        .collect();

    json_files.sort_by_key(|e| e.file_name());

    json_files.last().map(|e| e.path()).ok_or_else(|| {
        anyhow::anyhow!(
            "No allianz-lu-*.json files found in {}",
            output_dir.display()
        )
    })
}

struct AllianzCounts {
    cbus: i64,
    entities: i64,
    roles: i64,
}

async fn count_allianz_data(pool: &PgPool) -> Result<AllianzCounts> {
    let cbus = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%'"#,
    )
    .fetch_one(pool)
    .await?;

    let entities = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM "ob-poc".entities WHERE name ILIKE 'Allianz%'"#,
    )
    .fetch_one(pool)
    .await?;

    let roles = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(AllianzCounts {
        cbus,
        entities,
        roles,
    })
}

async fn clean_allianz_internal(pool: &PgPool) -> Result<()> {
    // Delete in reverse dependency order - all tables that reference cbus
    let cbu_dependent_tables = [
        // kyc schema
        (
            r#"DELETE FROM kyc.cases WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "kyc.cases",
        ),
        (
            r#"DELETE FROM kyc.share_classes WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "kyc.share_classes",
        ),
        // custody schema
        (
            r#"DELETE FROM custody.ssi_booking_rules WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "custody.ssi_booking_rules",
        ),
        (
            r#"DELETE FROM custody.cbu_ssi WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "custody.cbu_ssi",
        ),
        (
            r#"DELETE FROM custody.cbu_instrument_universe WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "custody.cbu_instrument_universe",
        ),
        (
            r#"DELETE FROM custody.isda_agreements WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "custody.isda_agreements",
        ),
        // ob-poc schema
        (
            r#"DELETE FROM "ob-poc".ubo_snapshot_comparisons WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "ubo_snapshot_comparisons",
        ),
        (
            r#"DELETE FROM "ob-poc".ubo_snapshots WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "ubo_snapshots",
        ),
        (
            r#"DELETE FROM "ob-poc".ubo_registry WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "ubo_registry",
        ),
        (
            r#"DELETE FROM "ob-poc".service_delivery_map WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "service_delivery_map",
        ),
        (
            r#"DELETE FROM "ob-poc".cbu_resource_instances WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "cbu_resource_instances",
        ),
        (
            r#"DELETE FROM "ob-poc".cbu_trading_profiles WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "cbu_trading_profiles",
        ),
        (
            r#"DELETE FROM "ob-poc".onboarding_plans WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "onboarding_plans",
        ),
        (
            r#"DELETE FROM "ob-poc".onboarding_requests WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "onboarding_requests",
        ),
        (
            r#"DELETE FROM "ob-poc".dsl_sessions WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "dsl_sessions",
        ),
        (
            r#"DELETE FROM "ob-poc".dsl_ob WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "dsl_ob",
        ),
        (
            r#"DELETE FROM "ob-poc".document_catalog WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "document_catalog",
        ),
        (
            r#"DELETE FROM "ob-poc".delegation_relationships WHERE applies_to_cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "delegation_relationships",
        ),
        (
            r#"DELETE FROM "ob-poc".client_allegations WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "client_allegations",
        ),
        (
            r#"DELETE FROM "ob-poc".cbu_evidence WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "cbu_evidence",
        ),
        (
            r#"DELETE FROM "ob-poc".cbu_creation_log WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "cbu_creation_log",
        ),
        (
            r#"DELETE FROM "ob-poc".cbu_change_log WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "cbu_change_log",
        ),
        (
            r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%')"#,
            "cbu_entity_roles",
        ),
    ];

    for (query, table_name) in cbu_dependent_tables {
        match sqlx::query(query).execute(pool).await {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    println!(
                        "    Deleted {} rows from {}",
                        result.rows_affected(),
                        table_name
                    );
                }
            }
            Err(e) => {
                // Log but continue - table might not exist or have no matching rows
                eprintln!("    Warning: {} - {}", table_name, e);
            }
        }
    }

    // Now delete the CBUs themselves
    let cbu_result = sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name ILIKE 'Allianz%'"#)
        .execute(pool)
        .await?;
    println!("    Deleted {} CBUs", cbu_result.rows_affected());

    // Delete from entity extension tables first (before base entities)
    let entity_extension_tables = [
        // Order matters due to parent_fund_id FK - delete share classes that reference Allianz funds first
        (
            r#"DELETE FROM "ob-poc".entity_share_classes WHERE parent_fund_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_share_classes (by parent_fund)",
        ),
        (
            r#"DELETE FROM "ob-poc".entity_share_classes WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_share_classes (by entity)",
        ),
        // entity_funds has parent_fund_id and master_fund_id FKs to entities - handle those first
        (
            r#"DELETE FROM "ob-poc".entity_funds WHERE parent_fund_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_funds (by parent_fund)",
        ),
        (
            r#"DELETE FROM "ob-poc".entity_funds WHERE master_fund_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_funds (by master_fund)",
        ),
        (
            r#"DELETE FROM "ob-poc".entity_funds WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_funds (by entity)",
        ),
        (
            r#"DELETE FROM "ob-poc".entity_manco WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_manco",
        ),
        (
            r#"DELETE FROM "ob-poc".entity_limited_companies WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_limited_companies",
        ),
        (
            r#"DELETE FROM "ob-poc".entity_proper_persons WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_proper_persons",
        ),
        (
            r#"DELETE FROM "ob-poc".entity_partnerships WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_partnerships",
        ),
        (
            r#"DELETE FROM "ob-poc".entity_trusts WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_trusts",
        ),
        // Delete ownership and control relationships
        (
            r#"DELETE FROM "ob-poc".ownership_relationships WHERE owner_entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%') OR owned_entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "ownership_relationships",
        ),
        (
            r#"DELETE FROM "ob-poc".control_relationships WHERE controller_entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%') OR controlled_entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "control_relationships",
        ),
        // Delete attribute observations
        (
            r#"DELETE FROM "ob-poc".attribute_observations WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "attribute_observations",
        ),
        // Delete entity KYC status
        (
            r#"DELETE FROM "ob-poc".entity_kyc_status WHERE entity_id IN (SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE 'Allianz%')"#,
            "entity_kyc_status",
        ),
    ];

    for (query, table_name) in entity_extension_tables {
        match sqlx::query(query).execute(pool).await {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    println!(
                        "    Deleted {} rows from {}",
                        result.rows_affected(),
                        table_name
                    );
                }
            }
            Err(e) => {
                eprintln!("    Warning: {} - {}", table_name, e);
            }
        }
    }

    // Finally delete entities
    let entity_result = sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE name ILIKE 'Allianz%'"#)
        .execute(pool)
        .await?;
    println!("    Deleted {} entities", entity_result.rows_affected());

    println!("  Cleanup complete");
    Ok(())
}

async fn get_entity_type_id(pool: &PgPool, type_code: &str) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = $1"#,
    )
    .bind(type_code)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow::anyhow!("Entity type not found: {}", type_code))
}

async fn create_entity(
    pool: &PgPool,
    name: &str,
    entity_type_id: Uuid,
    jurisdiction: Option<&str>,
) -> Result<Uuid> {
    let entity_id = Uuid::new_v4();

    // Insert into base entities table
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(entity_id)
    .bind(entity_type_id)
    .bind(name)
    .execute(pool)
    .await?;

    // Get the extension table for this entity type
    let table_name: String = sqlx::query_scalar(
        r#"SELECT table_name FROM "ob-poc".entity_types WHERE entity_type_id = $1"#,
    )
    .bind(entity_type_id)
    .fetch_one(pool)
    .await?;

    // Insert into extension table based on type
    match table_name.as_str() {
        "entity_limited_companies" => {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".entity_limited_companies
                (limited_company_id, entity_id, company_name, jurisdiction)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(Uuid::new_v4())
            .bind(entity_id)
            .bind(name)
            .bind(jurisdiction)
            .execute(pool)
            .await?;
        }
        "entity_funds" => {
            sqlx::query(
                r#"
                INSERT INTO "ob-poc".entity_funds
                (entity_id, jurisdiction, fund_structure_type, regulatory_status)
                VALUES ($1, $2, 'SICAV', 'UCITS')
                "#,
            )
            .bind(entity_id)
            .bind(jurisdiction)
            .execute(pool)
            .await?;
        }
        _ => {
            // For other types, just the base entity is enough
        }
    }

    Ok(entity_id)
}

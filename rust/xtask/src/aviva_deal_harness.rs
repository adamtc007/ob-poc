//! Aviva Deal Test Harness
//!
//! Creates a complete deal for Aviva Investors using DSL verbs only.
//! All operations are idempotent - safe to re-run.
//!
//! Creates:
//! - A deal for Aviva Investors
//! - Adds all products to the deal
//! - Creates rate cards with made-up rates for all products
//! - Creates 2 contracts linked to the deal
//! - Links Custody and Fund Accounting to Contract 1
//! - Links all other products to Contract 2

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::collections::HashMap;

/// Aviva deal constants
const AVIVA_CLIENT_GROUP_ID: &str = "22222222-2222-2222-2222-222222222222";
const AVIVA_DEAL_NAME: &str = "Aviva Investors Master Services Agreement 2024";

/// Products to be split between contracts
const CONTRACT_1_PRODUCTS: &[&str] = &["Custody", "CUSTODY", "Fund Accounting", "FUND_ACCOUNTING"];
#[allow(dead_code)] // Used for documentation, actual split logic uses CONTRACT_1_PRODUCTS
const CONTRACT_2_PRODUCTS: &[&str] = &[
    "Transfer Agency",
    "Collateral Management",
    "Markets FX",
    "Middle Office",
    "Alternatives",
];

/// Sample fee configurations per product type
fn get_fee_config(product_name: &str) -> Vec<FeeLineConfig> {
    match product_name.to_uppercase().as_str() {
        "CUSTODY" => vec![
            FeeLineConfig {
                fee_type: "CUSTODY",
                pricing_model: "BPS",
                rate_value: 3.5,
                minimum_fee: 25000.0,
                fee_basis: "AUM",
            },
            FeeLineConfig {
                fee_type: "SAFEKEEPING",
                pricing_model: "BPS",
                rate_value: 1.5,
                minimum_fee: 5000.0,
                fee_basis: "AUM",
            },
            FeeLineConfig {
                fee_type: "SETTLEMENT",
                pricing_model: "PER_TRANSACTION",
                rate_value: 15.0,
                minimum_fee: 0.0,
                fee_basis: "TRADE_COUNT",
            },
        ],
        "FUND_ACCOUNTING" | "FUND ACCOUNTING" => vec![
            FeeLineConfig {
                fee_type: "FUND_ACCOUNTING",
                pricing_model: "BPS",
                rate_value: 2.0,
                minimum_fee: 15000.0,
                fee_basis: "NAV",
            },
            FeeLineConfig {
                fee_type: "NAV_CALCULATION",
                pricing_model: "FLAT",
                rate_value: 500.0,
                minimum_fee: 0.0,
                fee_basis: "NAV",
            },
        ],
        "TRANSFER_AGENCY" | "TRANSFER AGENCY" => vec![
            FeeLineConfig {
                fee_type: "TRANSFER_AGENCY",
                pricing_model: "PER_TRANSACTION",
                rate_value: 8.0,
                minimum_fee: 10000.0,
                fee_basis: "TRADE_COUNT",
            },
            FeeLineConfig {
                fee_type: "INVESTOR_SERVICING",
                pricing_model: "FLAT",
                rate_value: 2500.0,
                minimum_fee: 0.0,
                fee_basis: "POSITION_COUNT",
            },
        ],
        "COLLATERAL_MANAGEMENT" | "COLLATERAL MANAGEMENT" => vec![FeeLineConfig {
            fee_type: "COLLATERAL_MANAGEMENT",
            pricing_model: "BPS",
            rate_value: 1.0,
            minimum_fee: 20000.0,
            fee_basis: "AUM",
        }],
        "MARKETS_FX" | "MARKETS FX" => vec![FeeLineConfig {
            fee_type: "FX_EXECUTION",
            pricing_model: "SPREAD",
            rate_value: 0.5,
            minimum_fee: 0.0,
            fee_basis: "TRADE_COUNT",
        }],
        "MIDDLE_OFFICE" | "MIDDLE OFFICE" => vec![FeeLineConfig {
            fee_type: "MIDDLE_OFFICE",
            pricing_model: "FLAT",
            rate_value: 35000.0,
            minimum_fee: 0.0,
            fee_basis: "AUM",
        }],
        "ALTERNATIVES" => vec![FeeLineConfig {
            fee_type: "ALTERNATIVES",
            pricing_model: "BPS",
            rate_value: 5.0,
            minimum_fee: 50000.0,
            fee_basis: "AUM",
        }],
        _ => vec![FeeLineConfig {
            fee_type: "DEFAULT",
            pricing_model: "BPS",
            rate_value: 2.5,
            minimum_fee: 10000.0,
            fee_basis: "AUM",
        }],
    }
}

#[derive(Debug, Clone)]
struct FeeLineConfig {
    fee_type: &'static str,
    pricing_model: &'static str,
    rate_value: f64,
    minimum_fee: f64,
    fee_basis: &'static str,
}

/// Results from the harness run
#[derive(Debug)]
pub struct AvivaHarnessResults {
    pub deal_id: Option<String>,
    pub contract_1_id: Option<String>,
    pub contract_2_id: Option<String>,
    pub products_added: Vec<String>,
    pub rate_cards_created: Vec<String>,
    pub steps_passed: usize,
    pub steps_failed: usize,
    pub errors: Vec<String>,
}

impl AvivaHarnessResults {
    fn new() -> Self {
        Self {
            deal_id: None,
            contract_1_id: None,
            contract_2_id: None,
            products_added: Vec::new(),
            rate_cards_created: Vec::new(),
            steps_passed: 0,
            steps_failed: 0,
            errors: Vec::new(),
        }
    }
}

/// Execute a DSL statement and return the result
async fn execute_dsl(pool: &PgPool, dsl: &str) -> Result<serde_json::Value> {
    use ob_poc::dsl_v2::executor::{DslExecutor, ExecutionResult};
    use ob_poc::ExecutionContext;

    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ExecutionContext::new();

    let results = executor
        .execute_dsl(dsl, &mut ctx)
        .await
        .context("DSL execution failed")?;

    if results.is_empty() {
        Ok(serde_json::json!({}))
    } else {
        match &results[0] {
            ExecutionResult::Uuid(uuid) => Ok(serde_json::json!(uuid.to_string())),
            ExecutionResult::Record(json) => Ok(json.clone()),
            ExecutionResult::RecordSet(records) => Ok(serde_json::json!(records)),
            ExecutionResult::Affected(count) => Ok(serde_json::json!({"affected": count})),
            ExecutionResult::Void => Ok(serde_json::json!({"status": "ok"})),
            ExecutionResult::EntityQuery(eq) => Ok(serde_json::json!({"count": eq.items.len()})),
            ExecutionResult::TemplateInvoked(ti) => {
                Ok(serde_json::json!({"template": ti.template_id}))
            }
            ExecutionResult::TemplateBatch(tb) => {
                Ok(serde_json::json!({"total": tb.total_items, "success": tb.success_count}))
            }
            ExecutionResult::BatchControl(bc) => Ok(serde_json::json!({"status": bc.status})),
        }
    }
}

/// Extract UUID from execution result
fn extract_uuid(result: &serde_json::Value) -> Option<String> {
    if let Some(uuid) = result.as_str() {
        return Some(uuid.to_string());
    }
    if let Some(obj) = result.as_object() {
        for key in &[
            "deal_id",
            "contract_id",
            "product_id",
            "rate_card_id",
            "deal_product_id",
            "line_id",
            "id",
            "uuid",
        ] {
            if let Some(v) = obj.get(*key) {
                if let Some(s) = v.as_str() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

/// Check if a deal already exists for Aviva
async fn find_existing_aviva_deal(pool: &PgPool) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"SELECT deal_id::text FROM "ob-poc".deals
           WHERE deal_name = $1
           AND deal_status NOT IN ('CANCELLED', 'OFFBOARDED')
           LIMIT 1"#,
    )
    .bind(AVIVA_DEAL_NAME)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id,)| id))
}

/// Get all products from the database
async fn get_all_products(pool: &PgPool) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> =
        sqlx::query_as(r#"SELECT name, product_id::text FROM "ob-poc".products ORDER BY name"#)
            .fetch_all(pool)
            .await?;

    Ok(rows.into_iter().collect())
}

/// Check if a contract already exists for Aviva with given reference prefix
async fn find_existing_contract(pool: &PgPool, reference_prefix: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"SELECT contract_id::text FROM "ob-poc".legal_contracts
           WHERE contract_reference LIKE $1
           AND status NOT IN ('TERMINATED', 'CANCELLED')
           LIMIT 1"#,
    )
    .bind(format!("{}%", reference_prefix))
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id,)| id))
}

/// Check if a product is already added to a deal
async fn is_product_in_deal(pool: &PgPool, deal_id: &str, product_id: &str) -> Result<bool> {
    let row: Option<(i32,)> = sqlx::query_as(
        r#"SELECT 1 FROM "ob-poc".deal_products
           WHERE deal_id = $1::uuid AND product_id = $2::uuid
           LIMIT 1"#,
    )
    .bind(deal_id)
    .bind(product_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

/// Check if a contract is already linked to a deal
async fn is_contract_linked_to_deal(
    pool: &PgPool,
    deal_id: &str,
    contract_id: &str,
) -> Result<bool> {
    let row: Option<(i32,)> = sqlx::query_as(
        r#"SELECT 1 FROM "ob-poc".deal_contracts
           WHERE deal_id = $1::uuid AND contract_id = $2::uuid
           LIMIT 1"#,
    )
    .bind(deal_id)
    .bind(contract_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

/// Check if a rate card exists for a deal/contract/product combination
async fn find_existing_rate_card(
    pool: &PgPool,
    deal_id: &str,
    contract_id: &str,
    product_id: &str,
) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"SELECT rate_card_id::text FROM "ob-poc".deal_rate_cards
           WHERE deal_id = $1::uuid
           AND contract_id = $2::uuid
           AND product_id = $3::uuid
           AND status NOT IN ('CANCELLED', 'SUPERSEDED')
           LIMIT 1"#,
    )
    .bind(deal_id)
    .bind(contract_id)
    .bind(product_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id,)| id))
}

/// Main harness function
pub async fn run_aviva_deal_harness(
    pool: PgPool,
    verbose: bool,
    dry_run: bool,
) -> Result<AvivaHarnessResults> {
    let mut results = AvivaHarnessResults::new();

    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("  Aviva Investors Deal Test Harness");
    println!("  Creating complete deal with products, contracts, and rate cards");
    println!("═══════════════════════════════════════════════════════════════════════════\n");

    if dry_run {
        println!("  Mode: DRY RUN - showing DSL without execution\n");
    } else {
        println!("  Mode: LIVE - executing DSL statements\n");
    }

    // =========================================================================
    // PHASE 1: Get or Create Deal
    // =========================================================================
    println!("PHASE 1: Deal Setup\n");

    let deal_id = if let Some(existing_id) = find_existing_aviva_deal(&pool).await? {
        println!("  ✓ Found existing deal: {}", existing_id);
        results.steps_passed += 1;
        existing_id
    } else {
        let deal_dsl = format!(
            r#"(deal.create
                :deal-name "{name}"
                :primary-client-group-id "{client_group}"
                :deal-reference "AVIVA-MSA-2024"
                :sales-owner "Aviva Relationship Team"
                :sales-team "EMEA Institutional"
                :estimated-revenue 2500000
                :currency-code "GBP"
                :notes "Master services agreement covering full custody and fund services suite")"#,
            name = AVIVA_DEAL_NAME,
            client_group = AVIVA_CLIENT_GROUP_ID
        );

        if verbose {
            println!("  DSL: {}", deal_dsl.replace('\n', "\n       "));
        }

        if dry_run {
            println!("  [DRY RUN] Would create deal: {}", AVIVA_DEAL_NAME);
            results.steps_passed += 1;
            "dry-run-deal-id".to_string()
        } else {
            match execute_dsl(&pool, &deal_dsl).await {
                Ok(result) => {
                    let id = extract_uuid(&result).unwrap_or_else(|| format!("{:?}", result));
                    println!("  ✓ Created deal: {}", id);
                    results.steps_passed += 1;
                    id
                }
                Err(e) => {
                    let err = format!("Failed to create deal: {}", e);
                    println!("  ✗ {}", err);
                    results.steps_failed += 1;
                    results.errors.push(err);
                    return Ok(results);
                }
            }
        }
    };
    results.deal_id = Some(deal_id.clone());

    // =========================================================================
    // PHASE 2: Create Contracts
    // =========================================================================
    println!("\nPHASE 2: Create Contracts\n");

    // Contract 1: Core Services (Custody + Fund Accounting)
    let contract_1_ref = "AVIVA-CORE-2024";
    let contract_1_id =
        if let Some(existing_id) = find_existing_contract(&pool, contract_1_ref).await? {
            println!(
                "  ✓ Found existing Contract 1 (Core Services): {}",
                existing_id
            );
            results.steps_passed += 1;
            existing_id
        } else {
            let contract_dsl = format!(
                r#"(contract.create
                :client "{client_group}"
                :reference "{ref}"
                :effective-date "2024-01-01")"#,
                client_group = AVIVA_CLIENT_GROUP_ID,
                ref = contract_1_ref
            );

            if verbose {
                println!("  DSL: {}", contract_dsl.replace('\n', "\n       "));
            }

            if dry_run {
                println!("  [DRY RUN] Would create Contract 1: {}", contract_1_ref);
                results.steps_passed += 1;
                "dry-run-contract-1".to_string()
            } else {
                match execute_dsl(&pool, &contract_dsl).await {
                    Ok(result) => {
                        let id = extract_uuid(&result).unwrap_or_else(|| format!("{:?}", result));
                        println!("  ✓ Created Contract 1 (Core Services): {}", id);
                        results.steps_passed += 1;
                        id
                    }
                    Err(e) => {
                        let err = format!("Failed to create Contract 1: {}", e);
                        println!("  ✗ {}", err);
                        results.steps_failed += 1;
                        results.errors.push(err);
                        "error".to_string()
                    }
                }
            }
        };
    results.contract_1_id = Some(contract_1_id.clone());

    // Contract 2: Ancillary Services
    let contract_2_ref = "AVIVA-ANCILLARY-2024";
    let contract_2_id =
        if let Some(existing_id) = find_existing_contract(&pool, contract_2_ref).await? {
            println!(
                "  ✓ Found existing Contract 2 (Ancillary Services): {}",
                existing_id
            );
            results.steps_passed += 1;
            existing_id
        } else {
            let contract_dsl = format!(
                r#"(contract.create
                :client "{client_group}"
                :reference "{ref}"
                :effective-date "2024-01-01")"#,
                client_group = AVIVA_CLIENT_GROUP_ID,
                ref = contract_2_ref
            );

            if verbose {
                println!("  DSL: {}", contract_dsl.replace('\n', "\n       "));
            }

            if dry_run {
                println!("  [DRY RUN] Would create Contract 2: {}", contract_2_ref);
                results.steps_passed += 1;
                "dry-run-contract-2".to_string()
            } else {
                match execute_dsl(&pool, &contract_dsl).await {
                    Ok(result) => {
                        let id = extract_uuid(&result).unwrap_or_else(|| format!("{:?}", result));
                        println!("  ✓ Created Contract 2 (Ancillary Services): {}", id);
                        results.steps_passed += 1;
                        id
                    }
                    Err(e) => {
                        let err = format!("Failed to create Contract 2: {}", e);
                        println!("  ✗ {}", err);
                        results.steps_failed += 1;
                        results.errors.push(err);
                        "error".to_string()
                    }
                }
            }
        };
    results.contract_2_id = Some(contract_2_id.clone());

    // =========================================================================
    // PHASE 3: Link Contracts to Deal
    // =========================================================================
    println!("\nPHASE 3: Link Contracts to Deal\n");

    // Link Contract 1
    let contract_1_already_linked = if dry_run {
        false
    } else {
        is_contract_linked_to_deal(&pool, &deal_id, &contract_1_id).await?
    };

    if !contract_1_already_linked {
        let link_dsl = format!(
            r#"(deal.add-contract :deal-id "{}" :contract-id "{}" :contract-role "PRIMARY")"#,
            deal_id, contract_1_id
        );

        if verbose {
            println!("  DSL: {}", link_dsl);
        }

        if dry_run {
            println!("  [DRY RUN] Would link Contract 1 as PRIMARY");
            results.steps_passed += 1;
        } else {
            match execute_dsl(&pool, &link_dsl).await {
                Ok(_) => {
                    println!("  ✓ Linked Contract 1 as PRIMARY");
                    results.steps_passed += 1;
                }
                Err(e) => {
                    // May already be linked - check and continue
                    if e.to_string().contains("duplicate")
                        || e.to_string().contains("already exists")
                    {
                        println!("  ✓ Contract 1 already linked");
                        results.steps_passed += 1;
                    } else {
                        let err = format!("Failed to link Contract 1: {}", e);
                        println!("  ✗ {}", err);
                        results.steps_failed += 1;
                        results.errors.push(err);
                    }
                }
            }
        }
    } else {
        println!("  ✓ Contract 1 already linked to deal");
        results.steps_passed += 1;
    }

    // Link Contract 2
    let contract_2_already_linked = if dry_run {
        false
    } else {
        is_contract_linked_to_deal(&pool, &deal_id, &contract_2_id).await?
    };

    if !contract_2_already_linked {
        let link_dsl = format!(
            r#"(deal.add-contract :deal-id "{}" :contract-id "{}" :contract-role "SCHEDULE")"#,
            deal_id, contract_2_id
        );

        if verbose {
            println!("  DSL: {}", link_dsl);
        }

        if dry_run {
            println!("  [DRY RUN] Would link Contract 2 as SCHEDULE");
            results.steps_passed += 1;
        } else {
            match execute_dsl(&pool, &link_dsl).await {
                Ok(_) => {
                    println!("  ✓ Linked Contract 2 as SCHEDULE");
                    results.steps_passed += 1;
                }
                Err(e) => {
                    if e.to_string().contains("duplicate")
                        || e.to_string().contains("already exists")
                    {
                        println!("  ✓ Contract 2 already linked");
                        results.steps_passed += 1;
                    } else {
                        let err = format!("Failed to link Contract 2: {}", e);
                        println!("  ✗ {}", err);
                        results.steps_failed += 1;
                        results.errors.push(err);
                    }
                }
            }
        }
    } else {
        println!("  ✓ Contract 2 already linked to deal");
        results.steps_passed += 1;
    }

    // =========================================================================
    // PHASE 4: Add Products to Deal
    // =========================================================================
    println!("\nPHASE 4: Add Products to Deal\n");

    let products = get_all_products(&pool).await?;
    println!("  Found {} products in database", products.len());

    for (product_name, product_id) in &products {
        let already_in_deal = if dry_run {
            false
        } else {
            is_product_in_deal(&pool, &deal_id, product_id).await?
        };

        if already_in_deal {
            println!("  ✓ {} already in deal scope", product_name);
            results.products_added.push(product_name.clone());
            results.steps_passed += 1;
            continue;
        }

        let add_product_dsl = format!(
            r#"(deal.add-product
                :deal-id "{deal_id}"
                :product-id "{product_id}"
                :product-status "PROPOSED"
                :indicative-revenue 300000)"#,
            deal_id = deal_id,
            product_id = product_id
        );

        if verbose {
            println!("  DSL: {}", add_product_dsl.replace('\n', "\n       "));
        }

        if dry_run {
            println!("  [DRY RUN] Would add product: {}", product_name);
            results.products_added.push(product_name.clone());
            results.steps_passed += 1;
        } else {
            match execute_dsl(&pool, &add_product_dsl).await {
                Ok(_) => {
                    println!("  ✓ Added product: {}", product_name);
                    results.products_added.push(product_name.clone());
                    results.steps_passed += 1;
                }
                Err(e) => {
                    if e.to_string().contains("duplicate")
                        || e.to_string().contains("already exists")
                    {
                        println!("  ✓ {} already in deal scope", product_name);
                        results.products_added.push(product_name.clone());
                        results.steps_passed += 1;
                    } else {
                        let err = format!("Failed to add {}: {}", product_name, e);
                        println!("  ✗ {}", err);
                        results.steps_failed += 1;
                        results.errors.push(err);
                    }
                }
            }
        }
    }

    // =========================================================================
    // PHASE 5: Create Rate Cards
    // =========================================================================
    println!("\nPHASE 5: Create Rate Cards\n");

    for (product_name, product_id) in &products {
        // Determine which contract this product should use
        let is_core_product = CONTRACT_1_PRODUCTS.iter().any(|p| {
            p.eq_ignore_ascii_case(product_name)
                || product_name.to_uppercase().replace(' ', "_")
                    == p.to_uppercase().replace(' ', "_")
        });

        let (contract_id, contract_label) = if is_core_product {
            (&contract_1_id, "Core Services")
        } else {
            (&contract_2_id, "Ancillary Services")
        };

        // Check if rate card already exists (skip check in dry_run mode)
        if !dry_run {
            if let Some(existing_rc) =
                find_existing_rate_card(&pool, &deal_id, contract_id, product_id).await?
            {
                println!(
                    "  ✓ Rate card for {} already exists: {}",
                    product_name, existing_rc
                );
                results.rate_cards_created.push(existing_rc);
                results.steps_passed += 1;
                continue;
            }
        }

        let rate_card_name = format!("{} Rate Card - Aviva 2024", product_name);
        let rate_card_dsl = format!(
            r#"(deal.create-rate-card
                :deal-id "{deal_id}"
                :contract-id "{contract_id}"
                :product-id "{product_id}"
                :rate-card-name "{name}"
                :effective-from "2024-01-01")"#,
            deal_id = deal_id,
            contract_id = contract_id,
            product_id = product_id,
            name = rate_card_name
        );

        if verbose {
            println!("  DSL: {}", rate_card_dsl.replace('\n', "\n       "));
        }

        let rate_card_id = if dry_run {
            println!(
                "  [DRY RUN] Would create rate card for {} ({})",
                product_name, contract_label
            );
            results.steps_passed += 1;
            format!(
                "dry-run-rc-{}",
                product_name.replace(' ', "-").to_lowercase()
            )
        } else {
            match execute_dsl(&pool, &rate_card_dsl).await {
                Ok(result) => {
                    let id = extract_uuid(&result).unwrap_or_else(|| format!("{:?}", result));
                    println!(
                        "  ✓ Created rate card for {} ({}): {}",
                        product_name, contract_label, id
                    );
                    results.rate_cards_created.push(id.clone());
                    results.steps_passed += 1;
                    id
                }
                Err(e) => {
                    let err = format!("Failed to create rate card for {}: {}", product_name, e);
                    println!("  ✗ {}", err);
                    results.steps_failed += 1;
                    results.errors.push(err);
                    continue;
                }
            }
        };

        // Add fee lines to the rate card
        let fee_configs = get_fee_config(product_name);
        for fee_config in fee_configs {
            let line_dsl = format!(
                r#"(deal.add-rate-card-line
                    :rate-card-id "{rate_card_id}"
                    :fee-type "{fee_type}"
                    :pricing-model "{pricing_model}"
                    :rate-value {rate_value}
                    :minimum-fee {minimum_fee}
                    :fee-basis "{fee_basis}"
                    :currency-code "GBP")"#,
                rate_card_id = rate_card_id,
                fee_type = fee_config.fee_type,
                pricing_model = fee_config.pricing_model,
                rate_value = fee_config.rate_value,
                minimum_fee = fee_config.minimum_fee,
                fee_basis = fee_config.fee_basis
            );

            if verbose {
                println!("    DSL: {}", line_dsl.replace('\n', "\n         "));
            }

            if dry_run {
                println!(
                    "    [DRY RUN] Would add fee line: {} @ {} {}",
                    fee_config.fee_type,
                    fee_config.rate_value,
                    if fee_config.pricing_model == "BPS" {
                        "bps"
                    } else {
                        fee_config.pricing_model
                    }
                );
                results.steps_passed += 1;
            } else {
                match execute_dsl(&pool, &line_dsl).await {
                    Ok(_) => {
                        println!(
                            "    ✓ Added fee line: {} @ {} {}",
                            fee_config.fee_type,
                            fee_config.rate_value,
                            if fee_config.pricing_model == "BPS" {
                                "bps"
                            } else {
                                fee_config.pricing_model
                            }
                        );
                        results.steps_passed += 1;
                    }
                    Err(e) => {
                        if e.to_string().contains("duplicate") {
                            println!("    ✓ Fee line already exists: {}", fee_config.fee_type);
                            results.steps_passed += 1;
                        } else {
                            let err =
                                format!("Failed to add fee line {}: {}", fee_config.fee_type, e);
                            println!("    ✗ {}", err);
                            results.steps_failed += 1;
                            results.errors.push(err);
                        }
                    }
                }
            }
        }
    }

    // =========================================================================
    // PHASE 6: Validation Summary
    // =========================================================================
    println!("\n═══════════════════════════════════════════════════════════════════════════");
    println!("  SUMMARY");
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("  Deal ID:      {:?}", results.deal_id);
    println!(
        "  Contract 1:   {:?} (Core Services: Custody, Fund Accounting)",
        results.contract_1_id
    );
    println!(
        "  Contract 2:   {:?} (Ancillary Services)",
        results.contract_2_id
    );
    println!(
        "  Products:     {} added to deal scope",
        results.products_added.len()
    );
    println!(
        "  Rate Cards:   {} created",
        results.rate_cards_created.len()
    );
    println!();
    println!("  Steps Passed: {} ✓", results.steps_passed);
    println!("  Steps Failed: {} ✗", results.steps_failed);

    if !results.errors.is_empty() {
        println!("\n  Errors:");
        for err in &results.errors {
            println!("    - {}", err);
        }
    }

    if results.steps_failed == 0 {
        println!("\n  ✓ All operations completed successfully!");
        println!("  This harness is idempotent - safe to re-run.");
    }

    println!("═══════════════════════════════════════════════════════════════════════════\n");

    Ok(results)
}

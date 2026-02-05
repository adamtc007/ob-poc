//! Deal Hierarchy Test Harness
//!
//! Tests the Deal → Products → Rate Cards DAG using DSL verbs only.
//! Validates precedence constraints are enforced at the database level.

use anyhow::{Context, Result};
use sqlx::PgPool;
use std::collections::HashMap;

/// Test result for a single step
#[derive(Debug)]
#[allow(dead_code)]
pub struct StepResult {
    pub step: String,
    pub dsl: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Results from the full harness run
#[derive(Debug)]
pub struct HarnessResults {
    pub steps: Vec<StepResult>,
    pub deal_id: Option<String>,
    pub contract_ids: Vec<String>,
    pub product_ids: Vec<String>,
    pub rate_card_ids: Vec<String>,
    pub passed: usize,
    pub failed: usize,
}

impl HarnessResults {
    fn new() -> Self {
        Self {
            steps: Vec::new(),
            deal_id: None,
            contract_ids: Vec::new(),
            product_ids: Vec::new(),
            rate_card_ids: Vec::new(),
            passed: 0,
            failed: 0,
        }
    }

    fn add_success(&mut self, step: &str, dsl: &str, output: &str) {
        self.steps.push(StepResult {
            step: step.to_string(),
            dsl: dsl.to_string(),
            success: true,
            output: output.to_string(),
            error: None,
        });
        self.passed += 1;
    }

    fn add_failure(&mut self, step: &str, dsl: &str, error: &str) {
        self.steps.push(StepResult {
            step: step.to_string(),
            dsl: dsl.to_string(),
            success: false,
            output: String::new(),
            error: Some(error.to_string()),
        });
        self.failed += 1;
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

    // Convert first result to JSON or return empty object
    if results.is_empty() {
        Ok(serde_json::json!({}))
    } else {
        // Convert ExecutionResult to JSON
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
    // Try different result formats
    if let Some(uuid) = result.as_str() {
        return Some(uuid.to_string());
    }
    if let Some(obj) = result.as_object() {
        // Look for common UUID field names
        for key in &[
            "deal_id",
            "contract_id",
            "product_id",
            "rate_card_id",
            "deal_product_id",
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

/// Run the deal hierarchy test harness
pub async fn run_deal_harness(
    pool: PgPool,
    verbose: bool,
    dry_run: bool,
    cleanup: bool,
) -> Result<HarnessResults> {
    let mut results = HarnessResults::new();

    println!("═══════════════════════════════════════════════════════════════");
    println!("  Deal Hierarchy Test Harness");
    println!("  Testing DAG: Deal → Products → Rate Cards");
    println!("═══════════════════════════════════════════════════════════════\n");

    if dry_run {
        println!("  🔍 DRY RUN - showing DSL without execution\n");
    }

    // =========================================================================
    // PHASE 1: Setup - Create prerequisite entities
    // =========================================================================
    println!("PHASE 1: Setup Prerequisites\n");

    // 1.1 Find or create a client group
    let client_group_id = get_or_create_test_client_group(&pool, verbose).await?;
    println!("  ✓ Client Group: {}", client_group_id);

    // 1.2 Find or create products (CUSTODY, FUND_ACCOUNTING)
    let product_ids = get_test_products(&pool, verbose).await?;
    println!("  ✓ Products: {} found", product_ids.len());
    results.product_ids = product_ids.values().cloned().collect();

    // =========================================================================
    // PHASE 2: Create Deal
    // =========================================================================
    println!("\nPHASE 2: Create Deal\n");

    let deal_dsl = format!(
        r#"(deal.create
            :deal-name "Test Deal Harness {timestamp}"
            :primary-client-group-id "{client_group}"
            :sales-owner "test-harness"
            :estimated-revenue 1000000
            :notes "Created by deal harness test")"#,
        timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        client_group = client_group_id
    );

    if verbose || dry_run {
        println!("  DSL: {}", deal_dsl.trim());
    }

    let deal_id = if dry_run {
        results.add_success("Create Deal", &deal_dsl, "DRY RUN - skipped");
        "dry-run-deal-id".to_string()
    } else {
        match execute_dsl(&pool, &deal_dsl).await {
            Ok(result) => {
                let id = extract_uuid(&result).unwrap_or_else(|| format!("{:?}", result));
                results.add_success("Create Deal", &deal_dsl, &id);
                println!("  ✓ Deal created: {}", id);
                id
            }
            Err(e) => {
                results.add_failure("Create Deal", &deal_dsl, &e.to_string());
                println!("  ✗ Failed: {}", e);
                return Ok(results);
            }
        }
    };
    results.deal_id = Some(deal_id.clone());

    // =========================================================================
    // PHASE 3: Add Products to Deal (scope)
    // =========================================================================
    println!("\nPHASE 3: Add Products to Deal Scope\n");

    for (product_name, product_id) in &product_ids {
        let add_product_dsl = format!(
            r#"(deal.add-product
                :deal-id "{deal_id}"
                :product-id "{product_id}"
                :product-status "PROPOSED"
                :indicative-revenue 500000)"#,
            deal_id = deal_id,
            product_id = product_id
        );

        if verbose || dry_run {
            println!("  DSL: {}", add_product_dsl.trim());
        }

        if dry_run {
            results.add_success(
                &format!("Add Product {}", product_name),
                &add_product_dsl,
                "DRY RUN",
            );
            println!("  ✓ Product {} added (dry run)", product_name);
        } else {
            match execute_dsl(&pool, &add_product_dsl).await {
                Ok(_) => {
                    results.add_success(
                        &format!("Add Product {}", product_name),
                        &add_product_dsl,
                        "OK",
                    );
                    println!("  ✓ Product {} added to deal scope", product_name);
                }
                Err(e) => {
                    results.add_failure(
                        &format!("Add Product {}", product_name),
                        &add_product_dsl,
                        &e.to_string(),
                    );
                    println!("  ✗ Failed to add {}: {}", product_name, e);
                }
            }
        }
    }

    // =========================================================================
    // PHASE 4: Create Contracts
    // =========================================================================
    println!("\nPHASE 4: Create Contracts\n");

    let mut contract_ids = Vec::new();
    for i in 1..=2 {
        let contract_dsl = format!(
            r#"(contract.create
                :client "{client_group}"
                :reference "TEST-MSA-{timestamp}-{i}"
                :effective-date "2024-01-01")"#,
            client_group = client_group_id,
            timestamp = chrono::Utc::now().format("%Y%m%d"),
            i = i
        );

        if verbose || dry_run {
            println!("  DSL: {}", contract_dsl.trim());
        }

        if dry_run {
            let fake_id = format!("dry-run-contract-{}", i);
            contract_ids.push(fake_id.clone());
            results.add_success(&format!("Create Contract {}", i), &contract_dsl, "DRY RUN");
            println!("  ✓ Contract {} created (dry run)", i);
        } else {
            match execute_dsl(&pool, &contract_dsl).await {
                Ok(result) => {
                    let id = extract_uuid(&result).unwrap_or_else(|| format!("{:?}", result));
                    contract_ids.push(id.clone());
                    results.add_success(&format!("Create Contract {}", i), &contract_dsl, &id);
                    println!("  ✓ Contract {} created: {}", i, id);
                }
                Err(e) => {
                    results.add_failure(
                        &format!("Create Contract {}", i),
                        &contract_dsl,
                        &e.to_string(),
                    );
                    println!("  ✗ Failed to create contract {}: {}", i, e);
                }
            }
        }
    }
    results.contract_ids = contract_ids.clone();

    // =========================================================================
    // PHASE 5: Link Contracts to Deal
    // =========================================================================
    println!("\nPHASE 5: Link Contracts to Deal\n");

    for (i, contract_id) in contract_ids.iter().enumerate() {
        let role = if i == 0 { "PRIMARY" } else { "SCHEDULE" };
        let link_dsl = format!(
            r#"(deal.add-contract
                :deal-id "{deal_id}"
                :contract-id "{contract_id}"
                :contract-role "{role}")"#,
            deal_id = deal_id,
            contract_id = contract_id,
            role = role
        );

        if verbose || dry_run {
            println!("  DSL: {}", link_dsl.trim());
        }

        if dry_run {
            results.add_success(&format!("Link Contract {}", i + 1), &link_dsl, "DRY RUN");
            println!("  ✓ Contract {} linked as {} (dry run)", i + 1, role);
        } else {
            match execute_dsl(&pool, &link_dsl).await {
                Ok(_) => {
                    results.add_success(&format!("Link Contract {}", i + 1), &link_dsl, "OK");
                    println!("  ✓ Contract {} linked to deal as {}", i + 1, role);
                }
                Err(e) => {
                    results.add_failure(
                        &format!("Link Contract {}", i + 1),
                        &link_dsl,
                        &e.to_string(),
                    );
                    println!("  ✗ Failed: {}", e);
                }
            }
        }
    }

    // =========================================================================
    // PHASE 6: Create Rate Cards (one per product per contract)
    // =========================================================================
    println!("\nPHASE 6: Create Rate Cards\n");

    // Use first contract for rate cards
    let primary_contract = contract_ids.first().cloned().unwrap_or_default();

    for (product_name, product_id) in &product_ids {
        let rate_card_dsl = format!(
            r#"(deal.create-rate-card
                :deal-id "{deal_id}"
                :contract-id "{contract_id}"
                :product-id "{product_id}"
                :rate-card-name "{product_name} Pricing v1"
                :effective-from "2024-01-01")"#,
            deal_id = deal_id,
            contract_id = primary_contract,
            product_id = product_id,
            product_name = product_name
        );

        if verbose || dry_run {
            println!("  DSL: {}", rate_card_dsl.trim());
        }

        if dry_run {
            let fake_id = format!("dry-run-rate-card-{}", product_name);
            results.rate_card_ids.push(fake_id);
            results.add_success(
                &format!("Create Rate Card for {}", product_name),
                &rate_card_dsl,
                "DRY RUN",
            );
            println!("  ✓ Rate card for {} created (dry run)", product_name);
        } else {
            match execute_dsl(&pool, &rate_card_dsl).await {
                Ok(result) => {
                    let id = extract_uuid(&result).unwrap_or_else(|| format!("{:?}", result));
                    results.rate_card_ids.push(id.clone());
                    results.add_success(
                        &format!("Create Rate Card for {}", product_name),
                        &rate_card_dsl,
                        &id,
                    );
                    println!("  ✓ Rate card for {} created: {}", product_name, id);
                }
                Err(e) => {
                    results.add_failure(
                        &format!("Create Rate Card for {}", product_name),
                        &rate_card_dsl,
                        &e.to_string(),
                    );
                    println!("  ✗ Failed: {}", e);
                }
            }
        }
    }

    // =========================================================================
    // PHASE 7: Add Rate Card Lines
    // =========================================================================
    println!("\nPHASE 7: Add Rate Card Lines\n");

    // Clone the rate_card_ids to avoid borrow checker issues
    let rate_card_ids_for_lines = results.rate_card_ids.clone();
    for (i, rate_card_id) in rate_card_ids_for_lines.iter().enumerate() {
        let fee_type = if i == 0 { "CUSTODY" } else { "FUND_ACCOUNTING" };

        let line_dsl = format!(
            r#"(deal.add-rate-card-line
                :rate-card-id "{rate_card_id}"
                :fee-type "{fee_type}"
                :pricing-model "BPS"
                :rate-value 5.0
                :minimum-fee 1000
                :fee-basis "AUM")"#,
            rate_card_id = rate_card_id,
            fee_type = fee_type
        );

        if verbose || dry_run {
            println!("  DSL: {}", line_dsl.trim());
        }

        if dry_run {
            results.add_success(
                &format!("Add Line to Rate Card {}", i + 1),
                &line_dsl,
                "DRY RUN",
            );
            println!("  ✓ Line added (dry run)");
        } else {
            match execute_dsl(&pool, &line_dsl).await {
                Ok(result) => {
                    let id = extract_uuid(&result).unwrap_or("OK".to_string());
                    results.add_success(
                        &format!("Add Line to Rate Card {}", i + 1),
                        &line_dsl,
                        &id,
                    );
                    println!("  ✓ Fee line {} added: {}", fee_type, id);
                }
                Err(e) => {
                    results.add_failure(
                        &format!("Add Line to Rate Card {}", i + 1),
                        &line_dsl,
                        &e.to_string(),
                    );
                    println!("  ✗ Failed: {}", e);
                }
            }
        }
    }

    // =========================================================================
    // PHASE 8: Propose and Agree Rate Cards
    // =========================================================================
    println!("\nPHASE 8: Negotiate Rate Cards\n");

    // Clone the rate_card_ids to avoid borrow checker issues
    let rate_card_ids_for_negotiate = results.rate_card_ids.clone();
    for (i, rate_card_id) in rate_card_ids_for_negotiate.iter().enumerate() {
        // Propose
        let propose_dsl = format!(
            r#"(deal.propose-rate-card :rate-card-id "{}")"#,
            rate_card_id
        );

        if !dry_run {
            match execute_dsl(&pool, &propose_dsl).await {
                Ok(_) => {
                    results.add_success(
                        &format!("Propose Rate Card {}", i + 1),
                        &propose_dsl,
                        "OK",
                    );
                    println!("  ✓ Rate card {} proposed", i + 1);
                }
                Err(e) => {
                    results.add_failure(
                        &format!("Propose Rate Card {}", i + 1),
                        &propose_dsl,
                        &e.to_string(),
                    );
                    println!("  ✗ Failed to propose: {}", e);
                    continue;
                }
            }
        }

        // Agree
        let agree_dsl = format!(r#"(deal.agree-rate-card :rate-card-id "{}")"#, rate_card_id);

        if !dry_run {
            match execute_dsl(&pool, &agree_dsl).await {
                Ok(_) => {
                    results.add_success(&format!("Agree Rate Card {}", i + 1), &agree_dsl, "OK");
                    println!("  ✓ Rate card {} agreed", i + 1);
                }
                Err(e) => {
                    results.add_failure(
                        &format!("Agree Rate Card {}", i + 1),
                        &agree_dsl,
                        &e.to_string(),
                    );
                    println!("  ✗ Failed to agree: {}", e);
                }
            }
        } else {
            results.add_success(
                &format!("Propose Rate Card {}", i + 1),
                &propose_dsl,
                "DRY RUN",
            );
            results.add_success(&format!("Agree Rate Card {}", i + 1), &agree_dsl, "DRY RUN");
            println!("  ✓ Rate card {} proposed and agreed (dry run)", i + 1);
        }
    }

    // =========================================================================
    // PHASE 9: Validation - Check all links exist
    // =========================================================================
    println!("\nPHASE 9: Validate Hierarchy Links\n");

    if !dry_run {
        // List products for deal
        let list_products_dsl = format!(r#"(deal.list-products :deal-id "{}")"#, deal_id);

        match execute_dsl(&pool, &list_products_dsl).await {
            Ok(result) => {
                println!("  ✓ Deal products: {:?}", result);
                results.add_success(
                    "Validate Deal Products",
                    &list_products_dsl,
                    &format!("{:?}", result),
                );
            }
            Err(e) => {
                println!("  ✗ Failed to list products: {}", e);
                results.add_failure("Validate Deal Products", &list_products_dsl, &e.to_string());
            }
        }

        // List contracts for deal
        let list_contracts_dsl = format!(r#"(deal.list-contracts :deal-id "{}")"#, deal_id);

        match execute_dsl(&pool, &list_contracts_dsl).await {
            Ok(result) => {
                println!("  ✓ Deal contracts: {:?}", result);
                results.add_success(
                    "Validate Deal Contracts",
                    &list_contracts_dsl,
                    &format!("{:?}", result),
                );
            }
            Err(e) => {
                println!("  ✗ Failed to list contracts: {}", e);
                results.add_failure(
                    "Validate Deal Contracts",
                    &list_contracts_dsl,
                    &e.to_string(),
                );
            }
        }

        // List rate cards for deal
        let list_rate_cards_dsl = format!(r#"(deal.list-rate-cards :deal-id "{}")"#, deal_id);

        match execute_dsl(&pool, &list_rate_cards_dsl).await {
            Ok(result) => {
                println!("  ✓ Deal rate cards: {:?}", result);
                results.add_success(
                    "Validate Deal Rate Cards",
                    &list_rate_cards_dsl,
                    &format!("{:?}", result),
                );
            }
            Err(e) => {
                println!("  ✗ Failed to list rate cards: {}", e);
                results.add_failure(
                    "Validate Deal Rate Cards",
                    &list_rate_cards_dsl,
                    &e.to_string(),
                );
            }
        }

        // Check active rate cards
        let active_rate_cards_dsl = format!(r#"(deal.active-rate-cards :deal-id "{}")"#, deal_id);

        match execute_dsl(&pool, &active_rate_cards_dsl).await {
            Ok(result) => {
                println!("  ✓ Active rate cards: {:?}", result);
                results.add_success(
                    "Validate Active Rate Cards",
                    &active_rate_cards_dsl,
                    &format!("{:?}", result),
                );
            }
            Err(e) => {
                println!("  ✗ Failed to list active rate cards: {}", e);
                results.add_failure(
                    "Validate Active Rate Cards",
                    &active_rate_cards_dsl,
                    &e.to_string(),
                );
            }
        }
    }

    // =========================================================================
    // PHASE 10: Test Precedence Constraint (should fail)
    // =========================================================================
    println!("\nPHASE 10: Test Precedence Constraint\n");
    println!("  Testing that duplicate AGREED rate cards are rejected...\n");

    if !dry_run && !results.rate_card_ids.is_empty() {
        // Try to create another rate card for the same product/contract and agree it
        // This should fail due to the precedence constraint
        let first_product = product_ids.values().next().unwrap();

        let duplicate_rate_card_dsl = format!(
            r#"(deal.create-rate-card
                :deal-id "{deal_id}"
                :contract-id "{contract_id}"
                :product-id "{product_id}"
                :rate-card-name "Duplicate Rate Card"
                :effective-from "2024-06-01")"#,
            deal_id = deal_id,
            contract_id = primary_contract,
            product_id = first_product
        );

        match execute_dsl(&pool, &duplicate_rate_card_dsl).await {
            Ok(result) => {
                let dup_id = extract_uuid(&result).unwrap_or_default();
                println!("  Created duplicate rate card: {}", dup_id);

                // Try to agree it - this should FAIL
                let agree_dup_dsl = format!(r#"(deal.agree-rate-card :rate-card-id "{}")"#, dup_id);

                match execute_dsl(&pool, &agree_dup_dsl).await {
                    Ok(_) => {
                        // This is BAD - constraint didn't work
                        results.add_failure(
                            "Precedence Constraint",
                            &agree_dup_dsl,
                            "CONSTRAINT VIOLATION: Duplicate AGREED rate card was allowed!",
                        );
                        println!("  ✗ CONSTRAINT FAILED: Duplicate AGREED rate card was allowed!");
                    }
                    Err(e) => {
                        // This is GOOD - constraint worked
                        results.add_success(
                            "Precedence Constraint",
                            &agree_dup_dsl,
                            &format!("Correctly rejected: {}", e),
                        );
                        println!("  ✓ Correctly rejected duplicate AGREED rate card");
                        println!("    Error: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("  Note: Could not create duplicate rate card: {}", e);
            }
        }
    }

    // =========================================================================
    // Cleanup (optional)
    // =========================================================================
    if cleanup && !dry_run {
        println!("\nCLEANUP: Removing test data\n");

        // Cancel the deal (soft delete)
        let cancel_dsl = format!(
            r#"(deal.cancel :deal-id "{}" :reason "Test harness cleanup")"#,
            deal_id
        );

        match execute_dsl(&pool, &cancel_dsl).await {
            Ok(_) => println!("  ✓ Deal cancelled"),
            Err(e) => println!("  ✗ Cleanup failed: {}", e),
        }
    }

    // =========================================================================
    // Summary
    // =========================================================================
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  SUMMARY");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Total steps: {}", results.steps.len());
    println!("  Passed: {} ✓", results.passed);
    println!("  Failed: {} ✗", results.failed);

    if let Some(ref deal_id) = results.deal_id {
        println!("\n  Created entities:");
        println!("    Deal: {}", deal_id);
        println!("    Contracts: {:?}", results.contract_ids);
        println!("    Rate Cards: {:?}", results.rate_card_ids);
    }

    if results.failed > 0 {
        println!("\n  ⚠ Some tests failed. Check output above for details.");
    } else {
        println!("\n  ✓ All tests passed!");
    }

    println!("═══════════════════════════════════════════════════════════════\n");

    Ok(results)
}

/// Get or create a test client group
async fn get_or_create_test_client_group(pool: &PgPool, _verbose: bool) -> Result<String> {
    // First try to find an existing client group
    let row: Option<(String,)> =
        sqlx::query_as(r#"SELECT id::text FROM "ob-poc".client_group LIMIT 1"#)
            .fetch_optional(pool)
            .await?;

    if let Some((id,)) = row {
        return Ok(id);
    }

    // If none exists, create one
    let id: (String,) = sqlx::query_as(
        r#"INSERT INTO "ob-poc".client_group (canonical_name)
           VALUES ('Test Client Group')
           RETURNING id::text"#,
    )
    .fetch_one(pool)
    .await?;

    Ok(id.0)
}

/// Get existing products for testing
async fn get_test_products(pool: &PgPool, _verbose: bool) -> Result<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT name, product_id::text
           FROM "ob-poc".products
           WHERE name IN ('CUSTODY', 'FUND_ACCOUNTING', 'TRANSFER_AGENCY')
           LIMIT 2"#,
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        // Create test products if none exist
        let custody: (String, String) = sqlx::query_as(
            r#"INSERT INTO "ob-poc".products (name, description, product_category)
               VALUES ('CUSTODY', 'Custody Services', 'CORE')
               ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
               RETURNING name, product_id::text"#,
        )
        .fetch_one(pool)
        .await?;

        let fund_acct: (String, String) = sqlx::query_as(
            r#"INSERT INTO "ob-poc".products (name, description, product_category)
               VALUES ('FUND_ACCOUNTING', 'Fund Accounting Services', 'CORE')
               ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
               RETURNING name, product_id::text"#,
        )
        .fetch_one(pool)
        .await?;

        let mut map = HashMap::new();
        map.insert(custody.0, custody.1);
        map.insert(fund_acct.0, fund_acct.1);
        return Ok(map);
    }

    Ok(rows.into_iter().collect())
}

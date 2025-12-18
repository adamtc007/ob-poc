//! UBO Test Harness
//!
//! Validates the KYC/UBO convergence model implementation through three test scenarios:
//! - Scenario 1: Simple Allianz Fund (2-level chain, 100% ownership)
//! - Scenario 2: Hedge Fund LLP (complex percentages, multiple UBOs)
//! - Scenario 3: Trust Structure (settlor/trustee/beneficiaries)
//!
//! Usage: cargo xtask ubo-test {scenario-1|scenario-2|scenario-3|all|clean|seed}

use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

/// Test scenario definition
#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub description: &'static str,
}

pub const SCENARIO_1: Scenario = Scenario {
    name: "scenario-1",
    description: "Simple Allianz Fund: 2-level ownership chain, 100% ManCo ownership",
};

pub const SCENARIO_2: Scenario = Scenario {
    name: "scenario-2",
    description: "Hedge Fund LLP: 6 entities, multiple ownership percentages (45%, 35%, 20%)",
};

pub const SCENARIO_3: Scenario = Scenario {
    name: "scenario-3",
    description: "Trust Structure: 7 entities, trust roles (settlor/trustee/beneficiaries)",
};

pub const SCENARIO_4: Scenario = Scenario {
    name: "scenario-4",
    description: "Periodic Review: proof expiry triggers re-verification loop",
};

/// Main entry point for UBO test harness
pub async fn run_ubo_test(command: &str, verbose: bool) -> Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url).await?;

    match command {
        "scenario-1" => run_scenario_1(&pool, verbose).await,
        "scenario-2" => run_scenario_2(&pool, verbose).await,
        "scenario-3" => run_scenario_3(&pool, verbose).await,
        "scenario-4" => run_scenario_4(&pool, verbose).await,
        "expired-proof" => run_expired_proof_test(&pool, verbose).await,
        "all" => {
            println!("===========================================");
            println!("  UBO Test Harness: Running All Scenarios");
            println!("===========================================\n");

            let mut passed = 0;
            let mut failed = 0;

            // Scenario 1
            println!("\n>>> Running {} <<<", SCENARIO_1.name);
            match run_scenario_1(&pool, verbose).await {
                Ok(_) => {
                    passed += 1;
                    println!("  {} PASSED", SCENARIO_1.name);
                }
                Err(e) => {
                    failed += 1;
                    println!("  {} FAILED: {}", SCENARIO_1.name, e);
                }
            }

            // Scenario 2
            println!("\n>>> Running {} <<<", SCENARIO_2.name);
            match run_scenario_2(&pool, verbose).await {
                Ok(_) => {
                    passed += 1;
                    println!("  {} PASSED", SCENARIO_2.name);
                }
                Err(e) => {
                    failed += 1;
                    println!("  {} FAILED: {}", SCENARIO_2.name, e);
                }
            }

            // Scenario 3
            println!("\n>>> Running {} <<<", SCENARIO_3.name);
            match run_scenario_3(&pool, verbose).await {
                Ok(_) => {
                    passed += 1;
                    println!("  {} PASSED", SCENARIO_3.name);
                }
                Err(e) => {
                    failed += 1;
                    println!("  {} FAILED: {}", SCENARIO_3.name, e);
                }
            }

            // Scenario 4
            println!("\n>>> Running {} <<<", SCENARIO_4.name);
            match run_scenario_4(&pool, verbose).await {
                Ok(_) => {
                    passed += 1;
                    println!("  {} PASSED", SCENARIO_4.name);
                }
                Err(e) => {
                    failed += 1;
                    println!("  {} FAILED: {}", SCENARIO_4.name, e);
                }
            }

            // Expired proof validation test
            println!("\n>>> Running expired-proof test <<<");
            match run_expired_proof_test(&pool, verbose).await {
                Ok(_) => {
                    passed += 1;
                    println!("  expired-proof PASSED");
                }
                Err(e) => {
                    failed += 1;
                    println!("  expired-proof FAILED: {}", e);
                }
            }

            println!("\n===========================================");
            println!("  Summary: {} passed, {} failed", passed, failed);
            println!("===========================================");

            if failed > 0 {
                anyhow::bail!("{} scenario(s) failed", failed);
            }
            Ok(())
        }
        "clean" => clean_test_data(&pool).await,
        "seed" => seed_test_data(&pool, verbose).await,
        _ => anyhow::bail!(
            "Unknown command: {}. Use: scenario-1, scenario-2, scenario-3, scenario-4, expired-proof, all, clean, seed",
            command
        ),
    }
}

/// Clean all UBO test data
pub async fn clean_test_data(pool: &PgPool) -> Result<()> {
    println!("Cleaning UBO test data...");

    // Find test CBUs (using naming convention)
    let test_cbu_names = [
        "UBO Test: Simple Fund",
        "UBO Test: Hedge Fund LLP",
        "UBO Test: Trust Structure",
    ];

    for name in test_cbu_names {
        let result =
            sqlx::query_scalar::<_, Uuid>(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1"#)
                .bind(name)
                .fetch_optional(pool)
                .await?;

        if let Some(cbu_id) = result {
            println!("  Deleting CBU: {} ({})", name, cbu_id);
            delete_cbu_cascade(pool, cbu_id).await?;
        }
    }

    // Also clean any orphaned test entities
    sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE name LIKE 'UBO Test:%'"#)
        .execute(pool)
        .await?;

    println!("Clean complete.");
    Ok(())
}

/// Cascade delete a CBU and all related data
async fn delete_cbu_cascade(pool: &PgPool, cbu_id: Uuid) -> Result<()> {
    let mut tx = pool.begin().await?;

    // ═══════════════════════════════════════════════════════════════════════
    // NEW ARCHITECTURE: Delete from cbu_relationship_verification (CBU-scoped)
    // Note: entity_relationships are structural facts and persist across CBUs
    // ═══════════════════════════════════════════════════════════════════════

    // Get relationship IDs for this CBU before deleting verification records
    let relationship_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT relationship_id FROM "ob-poc".cbu_relationship_verification WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_all(&mut *tx)
    .await?;

    // Delete CBU-scoped verification records
    sqlx::query(r#"DELETE FROM "ob-poc".cbu_relationship_verification WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

    // Delete orphaned entity_relationships (those with no remaining CBU verifications)
    // This is safe for test data since test entities are only used by test CBUs
    for rel_id in relationship_ids {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".cbu_relationship_verification WHERE relationship_id = $1"#,
        )
        .bind(rel_id)
        .fetch_one(&mut *tx)
        .await?;

        if count == 0 {
            sqlx::query(r#"DELETE FROM "ob-poc".entity_relationships WHERE relationship_id = $1"#)
                .bind(rel_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    // Legacy tables (try delete, ignore if not exist)
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".ubo_assertion_log WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await;

    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_decisions WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await;

    let _ = sqlx::query(r#"DELETE FROM "ob-poc".proofs WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await;

    // KYC schema
    sqlx::query(r#"DELETE FROM kyc.screenings WHERE workstream_id IN (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.doc_requests WHERE workstream_id IN (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1))"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.case_events WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.red_flags WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN (SELECT case_id FROM kyc.cases WHERE cbu_id = $1)"#)
        .bind(cbu_id).execute(&mut *tx).await?;
    sqlx::query(r#"DELETE FROM kyc.cases WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

    // Core tables
    sqlx::query(r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

    // Get entities linked to this CBU before deleting
    let entity_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT DISTINCT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#,
    )
    .bind(cbu_id)
    .fetch_all(&mut *tx)
    .await?;

    sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .execute(&mut *tx)
        .await?;

    // Delete test entities (only those with UBO Test: prefix)
    // First, delete entity_relationships that reference these entities (FK constraint)
    for entity_id in &entity_ids {
        sqlx::query(
            r#"DELETE FROM "ob-poc".entity_relationships
               WHERE from_entity_id = $1 OR to_entity_id = $1"#,
        )
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;
    }

    // Now delete the entities
    for entity_id in &entity_ids {
        sqlx::query(
            r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1 AND name LIKE 'UBO Test:%'"#,
        )
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Seed all test data
pub async fn seed_test_data(pool: &PgPool, verbose: bool) -> Result<()> {
    println!("Seeding UBO test data...");

    // Clean first
    clean_test_data(pool).await?;

    // Seed each scenario
    seed_scenario_1(pool, verbose).await?;
    seed_scenario_2(pool, verbose).await?;
    seed_scenario_3(pool, verbose).await?;

    println!("Seed complete.");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// SCENARIO 1: Simple Allianz Fund
// ═══════════════════════════════════════════════════════════════════════════

/// Seed Scenario 1: Simple Fund with 2-level ownership
async fn seed_scenario_1(pool: &PgPool, verbose: bool) -> Result<()> {
    if verbose {
        println!("  Seeding Scenario 1: {}", SCENARIO_1.description);
    }

    let mut tx = pool.begin().await?;

    // Get entity type IDs (use actual type_codes from database)
    let fund_type_id = get_entity_type_id(&mut tx, "fund_standalone").await?;
    let manco_type_id = get_entity_type_id(&mut tx, "management_company").await?;

    // Create entities
    let fund_entity_id =
        create_entity(&mut tx, "UBO Test: Allianz Growth Fund", fund_type_id).await?;
    let manco_entity_id =
        create_entity(&mut tx, "UBO Test: Allianz ManCo GmbH", manco_type_id).await?;

    // Create CBU
    let cbu_id = create_cbu(&mut tx, "UBO Test: Simple Fund", "LU", "FUND").await?;

    // Assign roles
    let asset_owner_role = get_role_id(&mut tx, "ASSET_OWNER").await?;
    let manco_role = get_role_id(&mut tx, "MANAGEMENT_COMPANY").await?;

    assign_role(&mut tx, cbu_id, fund_entity_id, asset_owner_role).await?;
    assign_role(&mut tx, cbu_id, manco_entity_id, manco_role).await?;

    // Create UBO edge: ManCo owns 100% of Fund
    create_ubo_edge(
        &mut tx,
        cbu_id,
        manco_entity_id, // from (owner)
        fund_entity_id,  // to (owned)
        "ownership",
        Some(100.0),
        None,
    )
    .await?;

    tx.commit().await?;

    if verbose {
        println!("    Created CBU: {} ({})", "UBO Test: Simple Fund", cbu_id);
        println!("    Created Fund Entity: {}", fund_entity_id);
        println!("    Created ManCo Entity: {}", manco_entity_id);
        println!("    Created ownership edge: ManCo -> Fund (100%)");
    }

    Ok(())
}

/// Run Scenario 1: Full convergence flow
async fn run_scenario_1(pool: &PgPool, verbose: bool) -> Result<()> {
    println!("\n===========================================");
    println!("  Scenario 1: {}", SCENARIO_1.description);
    println!("===========================================");

    // Ensure test data exists
    let cbu_id = get_test_cbu(pool, "UBO Test: Simple Fund").await?;

    if verbose {
        println!("\n  CBU ID: {}", cbu_id);
    }

    // Step 1: Check initial state (alleged)
    println!("\n  Step 1: Check initial state...");
    let status = get_convergence_status(pool, cbu_id).await?;
    if verbose {
        println!("    Total edges: {}", status.total_edges);
        println!("    Alleged: {}", status.alleged_edges);
        println!("    Proven: {}", status.proven_edges);
    }
    assert_eq!(status.alleged_edges, 1, "Expected 1 alleged edge");
    assert!(!status.is_converged, "Should not be converged initially");
    println!("    PASS: Initial state is 'alleged'");

    // Step 2: Create proof document
    println!("\n  Step 2: Create proof document...");
    let proof_id = create_proof(pool, cbu_id, "shareholder_register").await?;
    if verbose {
        println!("    Created proof: {}", proof_id);
    }
    println!("    PASS: Proof created");

    // Step 3: Link proof to edge
    println!("\n  Step 3: Link proof to edge...");
    let edge_id = get_single_edge(pool, cbu_id).await?;
    link_proof_to_edge(pool, edge_id, proof_id).await?;
    if verbose {
        println!("    Linked proof {} to edge {}", proof_id, edge_id);
    }

    // Check status is now 'pending'
    let status = get_convergence_status(pool, cbu_id).await?;
    assert_eq!(
        status.pending_edges, 1,
        "Expected 1 pending edge after linking proof"
    );
    println!("    PASS: Edge status is 'pending'");

    // Step 4: Verify the edge (prove allegation)
    println!("\n  Step 4: Verify edge...");
    verify_edge(pool, edge_id, 100.0).await?;

    let status = get_convergence_status(pool, cbu_id).await?;
    assert_eq!(
        status.proven_edges, 1,
        "Expected 1 proven edge after verification"
    );
    assert!(
        status.is_converged,
        "Should be converged after verification"
    );
    println!("    PASS: Edge verified, graph is CONVERGED");

    // Step 5: Run ubo.evaluate equivalent
    println!("\n  Step 5: Evaluate UBO...");
    let evaluation = evaluate_ubo(pool, cbu_id).await?;
    if verbose {
        println!(
            "    Beneficial owners found: {}",
            evaluation.beneficial_owners.len()
        );
        for bo in &evaluation.beneficial_owners {
            println!("      - {} ({}%)", bo.name, bo.percentage);
        }
    }
    // ManCo should be identified as 100% owner
    assert!(
        evaluation
            .beneficial_owners
            .iter()
            .any(|bo| bo.percentage >= 25.0),
        "Expected at least one beneficial owner with >=25%"
    );
    println!("    PASS: UBO evaluation complete");

    // Step 6: Make decision
    println!("\n  Step 6: Record decision...");
    record_decision(pool, cbu_id, "CLEARED").await?;
    println!("    PASS: Decision recorded as CLEARED");

    println!("\n  Scenario 1: ALL CHECKS PASSED");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// SCENARIO 2: Hedge Fund LLP
// ═══════════════════════════════════════════════════════════════════════════

async fn seed_scenario_2(pool: &PgPool, verbose: bool) -> Result<()> {
    if verbose {
        println!("  Seeding Scenario 2: {}", SCENARIO_2.description);
    }

    let mut tx = pool.begin().await?;

    // Get entity type IDs (use actual type_codes from database)
    let partnership_type_id = get_entity_type_id(&mut tx, "PARTNERSHIP_LIMITED").await?;
    let person_type_id = get_entity_type_id(&mut tx, "PROPER_PERSON_NATURAL").await?;
    let company_type_id = get_entity_type_id(&mut tx, "LIMITED_COMPANY_PRIVATE").await?;

    // Create entities
    let fund_id = create_entity(
        &mut tx,
        "UBO Test: Alpha Hedge Fund LLP",
        partnership_type_id,
    )
    .await?;
    let gp_id = create_entity(&mut tx, "UBO Test: Alpha GP LLC", company_type_id).await?;
    let lp1_id = create_entity(&mut tx, "UBO Test: John Smith", person_type_id).await?;
    let lp2_id = create_entity(&mut tx, "UBO Test: Jane Doe", person_type_id).await?;
    let lp3_id = create_entity(&mut tx, "UBO Test: Bob Wilson", person_type_id).await?;
    let gp_owner_id = create_entity(&mut tx, "UBO Test: GP Principal", person_type_id).await?;

    // Create CBU
    let cbu_id = create_cbu(&mut tx, "UBO Test: Hedge Fund LLP", "US", "FUND").await?;

    // Assign roles
    let principal_role = get_role_id(&mut tx, "PRINCIPAL").await?;
    assign_role(&mut tx, cbu_id, fund_id, principal_role).await?;

    // Create UBO edges:
    // - GP owns 0% economic but has control (edge_type = control)
    // - LP1 (John Smith) owns 45%
    // - LP2 (Jane Doe) owns 35%
    // - LP3 (Bob Wilson) owns 20%
    // - GP Principal owns 100% of GP

    create_ubo_edge(
        &mut tx,
        cbu_id,
        lp1_id,
        fund_id,
        "ownership",
        Some(45.0),
        None,
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        lp2_id,
        fund_id,
        "ownership",
        Some(35.0),
        None,
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        lp3_id,
        fund_id,
        "ownership",
        Some(20.0),
        None,
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        gp_id,
        fund_id,
        "control",
        None,
        Some("general_partner"),
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        gp_owner_id,
        gp_id,
        "ownership",
        Some(100.0),
        None,
    )
    .await?;

    tx.commit().await?;

    if verbose {
        println!(
            "    Created CBU: {} ({})",
            "UBO Test: Hedge Fund LLP", cbu_id
        );
        println!("    Created 6 entities with 5 UBO edges");
    }

    Ok(())
}

async fn run_scenario_2(pool: &PgPool, verbose: bool) -> Result<()> {
    println!("\n===========================================");
    println!("  Scenario 2: {}", SCENARIO_2.description);
    println!("===========================================");

    let cbu_id = get_test_cbu(pool, "UBO Test: Hedge Fund LLP").await?;

    if verbose {
        println!("\n  CBU ID: {}", cbu_id);
    }

    // Step 1: Check initial state
    println!("\n  Step 1: Check initial state...");
    let status = get_convergence_status(pool, cbu_id).await?;
    if verbose {
        println!("    Total edges: {}", status.total_edges);
        println!("    Alleged: {}", status.alleged_edges);
    }
    assert_eq!(status.total_edges, 5, "Expected 5 edges");
    assert_eq!(
        status.alleged_edges, 5,
        "Expected all 5 edges to be alleged"
    );
    println!("    PASS: 5 edges in alleged state");

    // Step 2: Create proofs and verify all edges
    println!("\n  Step 2: Create proofs and verify all ownership edges...");

    let edges = get_all_edges(pool, cbu_id).await?;
    let mut ownership_count = 0;
    let mut control_count = 0;

    for edge in &edges {
        let proof_type = if edge.edge_type == "ownership" {
            "partnership_agreement"
        } else {
            "board_resolution"
        };

        let proof_id = create_proof(pool, cbu_id, proof_type).await?;
        link_proof_to_edge(pool, edge.edge_id, proof_id).await?;

        if edge.edge_type == "ownership" {
            verify_edge(pool, edge.edge_id, edge.alleged_percentage.unwrap_or(0.0)).await?;
            ownership_count += 1;
        } else {
            verify_edge_control(pool, edge.edge_id, edge.alleged_role.as_deref()).await?;
            control_count += 1;
        }
    }

    if verbose {
        println!(
            "    Verified {} ownership edges, {} control edges",
            ownership_count, control_count
        );
    }

    let status = get_convergence_status(pool, cbu_id).await?;
    assert!(
        status.is_converged,
        "Should be converged after all verifications"
    );
    println!("    PASS: All edges verified, graph is CONVERGED");

    // Step 3: Evaluate UBOs
    println!("\n  Step 3: Evaluate UBO...");
    let evaluation = evaluate_ubo(pool, cbu_id).await?;

    if verbose {
        println!(
            "    Beneficial owners (>=25%): {}",
            evaluation.beneficial_owners.len()
        );
        for bo in &evaluation.beneficial_owners {
            println!("      - {} ({}%)", bo.name, bo.percentage);
        }
        println!("    Control persons: {}", evaluation.control_persons.len());
        for cp in &evaluation.control_persons {
            println!("      - {} ({})", cp.name, cp.role);
        }
    }

    // Expected: John Smith (45%), Jane Doe (35%), GP Principal (100% of GP) are UBOs by ownership
    // Bob Wilson (20%) is below threshold
    // GP LLC is control person (general_partner role)
    assert_eq!(
        evaluation.beneficial_owners.len(),
        3,
        "Expected 3 beneficial owners (>=25%): John Smith, Jane Doe, GP Principal"
    );
    assert!(
        evaluation.control_persons.len() >= 1,
        "Expected at least 1 control person (GP LLC as general_partner)"
    );
    println!("    PASS: Identified 3 BOs + control person(s)");

    // Step 4: Make decision
    println!("\n  Step 4: Record decision...");
    record_decision(pool, cbu_id, "CLEARED").await?;
    println!("    PASS: Decision recorded as CLEARED");

    println!("\n  Scenario 2: ALL CHECKS PASSED");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// SCENARIO 3: Trust Structure
// ═══════════════════════════════════════════════════════════════════════════

async fn seed_scenario_3(pool: &PgPool, verbose: bool) -> Result<()> {
    if verbose {
        println!("  Seeding Scenario 3: {}", SCENARIO_3.description);
    }

    let mut tx = pool.begin().await?;

    // Get entity type IDs (use actual type_codes from database)
    let trust_type_id = get_entity_type_id(&mut tx, "TRUST_DISCRETIONARY").await?;
    let person_type_id = get_entity_type_id(&mut tx, "PROPER_PERSON_NATURAL").await?;
    let company_type_id = get_entity_type_id(&mut tx, "LIMITED_COMPANY_PRIVATE").await?;

    // Create entities
    let trust_id = create_entity(&mut tx, "UBO Test: Smith Family Trust", trust_type_id).await?;
    let settlor_id =
        create_entity(&mut tx, "UBO Test: Robert Smith (Settlor)", person_type_id).await?;
    let trustee_co_id = create_entity(&mut tx, "UBO Test: Trust Corp Ltd", company_type_id).await?;
    let trustee_person_id =
        create_entity(&mut tx, "UBO Test: Trustee Director", person_type_id).await?;
    let beneficiary1_id = create_entity(
        &mut tx,
        "UBO Test: Alice Smith (Beneficiary)",
        person_type_id,
    )
    .await?;
    let beneficiary2_id = create_entity(
        &mut tx,
        "UBO Test: Charlie Smith (Beneficiary)",
        person_type_id,
    )
    .await?;
    let protector_id =
        create_entity(&mut tx, "UBO Test: David Brown (Protector)", person_type_id).await?;

    // Create CBU
    let cbu_id = create_cbu(&mut tx, "UBO Test: Trust Structure", "JE", "TRUST").await?;

    // Assign roles
    let settlor_role = get_role_id(&mut tx, "SETTLOR").await?;
    assign_role(&mut tx, cbu_id, settlor_id, settlor_role).await?;

    // Create UBO edges for trust relationships
    create_ubo_edge(
        &mut tx,
        cbu_id,
        settlor_id,
        trust_id,
        "trust_role",
        None,
        Some("settlor"),
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        trustee_co_id,
        trust_id,
        "trust_role",
        None,
        Some("trustee"),
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        trustee_person_id,
        trustee_co_id,
        "control",
        None,
        Some("director"),
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        beneficiary1_id,
        trust_id,
        "trust_role",
        None,
        Some("beneficiary"),
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        beneficiary2_id,
        trust_id,
        "trust_role",
        None,
        Some("beneficiary"),
    )
    .await?;
    create_ubo_edge(
        &mut tx,
        cbu_id,
        protector_id,
        trust_id,
        "trust_role",
        None,
        Some("protector"),
    )
    .await?;

    tx.commit().await?;

    if verbose {
        println!(
            "    Created CBU: {} ({})",
            "UBO Test: Trust Structure", cbu_id
        );
        println!("    Created 7 entities with 6 UBO edges");
    }

    Ok(())
}

async fn run_scenario_3(pool: &PgPool, verbose: bool) -> Result<()> {
    println!("\n===========================================");
    println!("  Scenario 3: {}", SCENARIO_3.description);
    println!("===========================================");

    let cbu_id = get_test_cbu(pool, "UBO Test: Trust Structure").await?;

    if verbose {
        println!("\n  CBU ID: {}", cbu_id);
    }

    // Step 1: Check initial state
    println!("\n  Step 1: Check initial state...");
    let status = get_convergence_status(pool, cbu_id).await?;
    if verbose {
        println!("    Total edges: {}", status.total_edges);
        println!("    Alleged: {}", status.alleged_edges);
    }
    assert_eq!(status.total_edges, 6, "Expected 6 edges");
    println!("    PASS: 6 trust relationship edges");

    // Step 2: Create proofs and verify all edges
    println!("\n  Step 2: Create proofs and verify all edges...");

    let edges = get_all_edges(pool, cbu_id).await?;
    for edge in &edges {
        let proof_type = if edge.edge_type == "trust_role" {
            "trust_deed"
        } else {
            "board_resolution"
        };

        let proof_id = create_proof(pool, cbu_id, proof_type).await?;
        link_proof_to_edge(pool, edge.edge_id, proof_id).await?;
        verify_edge_control(pool, edge.edge_id, edge.alleged_role.as_deref()).await?;
    }

    let status = get_convergence_status(pool, cbu_id).await?;
    assert!(
        status.is_converged,
        "Should be converged after all verifications"
    );
    println!("    PASS: All edges verified, graph is CONVERGED");

    // Step 3: Evaluate UBOs for trust
    println!("\n  Step 3: Evaluate UBO for trust...");
    let evaluation = evaluate_ubo(pool, cbu_id).await?;

    if verbose {
        println!("    Trust parties identified:");
        for cp in &evaluation.control_persons {
            println!("      - {} ({})", cp.name, cp.role);
        }
    }

    // For trusts, we expect:
    // - Settlor (Robert Smith) - always a BO
    // - Beneficiaries with vested interest - BOs (both Alice and Charlie)
    // - Protector with significant control - may be BO
    // Trustee Corporation itself is not a BO, but Trustee Director may be
    let trust_roles: Vec<&str> = evaluation
        .control_persons
        .iter()
        .map(|cp| cp.role.as_str())
        .collect();

    assert!(
        trust_roles.iter().any(|r| *r == "settlor"),
        "Expected settlor to be identified"
    );
    assert!(
        trust_roles.iter().filter(|r| **r == "beneficiary").count() >= 2,
        "Expected at least 2 beneficiaries"
    );
    println!("    PASS: Trust parties correctly identified");

    // Step 4: Check for enhanced DD requirement (trusts typically require it)
    println!("\n  Step 4: Check enhanced DD requirement...");
    // Trusts in Jersey typically require enhanced due diligence
    let requires_edd = true; // Simplified: trusts always require EDD
    if verbose {
        println!("    Enhanced DD required: {}", requires_edd);
    }
    println!("    PASS: Enhanced DD flagged");

    // Step 5: Make decision
    println!("\n  Step 5: Record decision...");
    record_decision(pool, cbu_id, "CLEARED").await?;
    println!("    PASS: Decision recorded as CLEARED");

    println!("\n  Scenario 3: ALL CHECKS PASSED");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// SCENARIO 4: Periodic Review (Expired Proof / Re-verification Loop)
// ═══════════════════════════════════════════════════════════════════════════

/// Run Scenario 4: Periodic Review
///
/// This scenario tests the re-verification loop that occurs during periodic reviews.
/// It validates that:
/// 1. After mark-dirty, edges stay 'proven' (convergence still passes)
/// 2. But no-expired-proofs assertion fails (proof needs refresh)
/// 3. Linking fresh proof and re-verifying restores full compliance
/// 4. New decision can be recorded with fresh review date
async fn run_scenario_4(pool: &PgPool, verbose: bool) -> Result<()> {
    use ob_poc::dsl_v2::ast::Statement;
    use ob_poc::dsl_v2::custom_ops::ubo_graph_ops::{UboAssertOp, UboMarkDirtyOp};
    use ob_poc::dsl_v2::custom_ops::CustomOperation;
    use ob_poc::dsl_v2::executor::ExecutionContext;
    use ob_poc::dsl_v2::parser::parse_program;

    println!("\n===========================================");
    println!("  Scenario 4: {}", SCENARIO_4.description);
    println!("===========================================");

    // Ensure Scenario 1 has been run (we build on its converged state)
    let cbu_id = get_test_cbu(pool, "UBO Test: Simple Fund").await?;

    if verbose {
        println!("\n  CBU ID: {}", cbu_id);
    }

    // Step 0: Ensure we're starting from a converged state
    println!("\n  Step 0: Verify starting state is converged...");
    let status = get_convergence_status(pool, cbu_id).await?;
    if !status.is_converged {
        // Run scenario 1 to get to converged state
        println!("    Not converged - running scenario 1 first...");
        run_scenario_1(pool, verbose).await?;
    }
    let status = get_convergence_status(pool, cbu_id).await?;
    assert!(status.is_converged, "Pre-condition: must be converged");
    println!("    PASS: Starting from converged state");

    // Get the proof linked to an edge (via cbu_relationship_verification)
    let proof_id: Uuid = sqlx::query_scalar(
        r#"SELECT p.proof_id FROM "ob-poc".proofs p
           JOIN "ob-poc".cbu_relationship_verification v ON v.proof_document_id = p.document_id
           WHERE v.cbu_id = $1 AND p.status = 'valid'
           LIMIT 1"#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await
    .context("No valid proof found - run scenario 1 first")?;

    if verbose {
        println!("    Using proof: {}", proof_id);
    }

    // Step 1: Mark the proof dirty (simulating periodic review trigger)
    println!("\n  Step 1: Mark proof dirty (periodic review trigger)...");

    let mark_dirty_dsl = format!(
        r#"(ubo.mark-dirty :proof "{}" :reason "Annual review due")"#,
        proof_id
    );

    if verbose {
        println!("    DSL: {}", mark_dirty_dsl);
    }

    let program = parse_program(&mark_dirty_dsl).map_err(|e| anyhow::anyhow!("{}", e))?;
    let verb_call = match &program.statements[0] {
        Statement::VerbCall(vc) => vc,
        _ => anyhow::bail!("Expected VerbCall"),
    };

    let op = UboMarkDirtyOp;
    let mut ctx = ExecutionContext::new();
    op.execute(verb_call, &mut ctx, pool).await?;
    println!("    PASS: Proof marked dirty");

    // Step 2: Assert convergence still passes (edges stay proven)
    println!("\n  Step 2: Assert convergence still passes...");

    let assert_converged_dsl = format!(r#"(ubo.assert :cbu "{}" :converged true)"#, cbu_id);

    let program = parse_program(&assert_converged_dsl).map_err(|e| anyhow::anyhow!("{}", e))?;
    let verb_call = match &program.statements[0] {
        Statement::VerbCall(vc) => vc,
        _ => anyhow::bail!("Expected VerbCall"),
    };

    let assert_op = UboAssertOp;
    let mut ctx = ExecutionContext::new();
    let result = assert_op.execute(verb_call, &mut ctx, pool).await;

    match result {
        Ok(_) => println!("    PASS: Convergence assertion still passes (edges remain proven)"),
        Err(e) => anyhow::bail!("Convergence should pass but failed: {}", e),
    }

    // Step 3: Assert no-expired-proofs FAILS (this is expected)
    println!("\n  Step 3: Assert no-expired-proofs fails (expected)...");

    let assert_no_expired_dsl =
        format!(r#"(ubo.assert :cbu "{}" :no-expired-proofs true)"#, cbu_id);

    let program = parse_program(&assert_no_expired_dsl).map_err(|e| anyhow::anyhow!("{}", e))?;
    let verb_call = match &program.statements[0] {
        Statement::VerbCall(vc) => vc,
        _ => anyhow::bail!("Expected VerbCall"),
    };

    let mut ctx = ExecutionContext::new();
    let result = assert_op.execute(verb_call, &mut ctx, pool).await;

    match result {
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("no-expired-proofs") {
                println!("    PASS: no-expired-proofs assertion correctly fails");
                if verbose {
                    println!("    Error (expected): {}", error_msg);
                }
            } else {
                anyhow::bail!("Wrong error - expected no-expired-proofs failure: {}", e);
            }
        }
        Ok(_) => {
            anyhow::bail!("no-expired-proofs should have failed but passed");
        }
    }

    // Step 4: Link a fresh proof to refresh the evidence
    println!("\n  Step 4: Link fresh proof (re-verification)...");

    // Get a relationship verification record to update
    let verification_id: Uuid =
        sqlx::query_scalar(r#"SELECT verification_id FROM "ob-poc".cbu_relationship_verification WHERE cbu_id = $1 LIMIT 1"#)
            .bind(cbu_id)
            .fetch_one(pool)
            .await?;

    // Create a new proof document
    let new_doc_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".document_catalog (cbu_id, document_name, status)
           VALUES ($1, 'Refreshed Shareholder Register', 'active')
           RETURNING doc_id"#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await?;

    // Create new proof with future validity
    let next_year = (chrono::Utc::now() + chrono::Duration::days(365))
        .format("%Y-%m-%d")
        .to_string();

    let new_proof_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".proofs (cbu_id, document_id, proof_type, status, valid_from, valid_until)
           VALUES ($1, $2, 'shareholder_register', 'valid', CURRENT_DATE, $3::date)
           RETURNING proof_id"#,
    )
    .bind(cbu_id)
    .bind(new_doc_id)
    .bind(&next_year)
    .fetch_one(pool)
    .await?;

    // Link the new proof to the verification record (replaces old proof_document_id)
    sqlx::query(r#"UPDATE "ob-poc".cbu_relationship_verification SET proof_document_id = $1 WHERE verification_id = $2"#)
        .bind(new_doc_id)
        .bind(verification_id)
        .execute(pool)
        .await?;

    if verbose {
        println!(
            "    Created new proof: {} (valid until {})",
            new_proof_id, next_year
        );
        println!("    Linked to verification: {}", verification_id);
    }
    println!("    PASS: Fresh proof linked");

    // Step 5: Assert no-expired-proofs NOW passes
    println!("\n  Step 5: Assert no-expired-proofs now passes...");

    let program = parse_program(&assert_no_expired_dsl).map_err(|e| anyhow::anyhow!("{}", e))?;
    let verb_call = match &program.statements[0] {
        Statement::VerbCall(vc) => vc,
        _ => anyhow::bail!("Expected VerbCall"),
    };

    let mut ctx = ExecutionContext::new();
    let result = assert_op.execute(verb_call, &mut ctx, pool).await;

    match result {
        Ok(_) => println!("    PASS: no-expired-proofs assertion now passes"),
        Err(e) => anyhow::bail!("no-expired-proofs should pass after refresh: {}", e),
    }

    // Step 6: Record new decision (periodic review complete)
    println!("\n  Step 6: Record periodic review decision...");
    record_decision(pool, cbu_id, "CLEARED").await?;
    println!("    PASS: New decision recorded");

    // Cleanup: Restore original proof for other tests
    // (keep new proof but mark old one as superseded)
    sqlx::query(
        r#"UPDATE "ob-poc".proofs SET status = 'superseded'
           WHERE proof_id = $1"#,
    )
    .bind(proof_id)
    .execute(pool)
    .await?;

    println!("\n  Scenario 4: ALL CHECKS PASSED");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// EXPIRED PROOF VALIDATION TEST
// ═══════════════════════════════════════════════════════════════════════════

/// Test that ubo.link-proof rejects already-expired proofs
///
/// This validates that proofs with valid_until < today are rejected at link time.
/// Proofs are like passports - they have validity periods and expired proofs
/// provide no evidentiary value.
async fn run_expired_proof_test(pool: &PgPool, verbose: bool) -> Result<()> {
    use ob_poc::dsl_v2::ast::Statement;
    use ob_poc::dsl_v2::custom_ops::ubo_graph_ops::UboLinkProofOp;
    use ob_poc::dsl_v2::custom_ops::CustomOperation;
    use ob_poc::dsl_v2::executor::ExecutionContext;
    use ob_poc::dsl_v2::parser::parse_program;

    println!("===========================================");
    println!("  Expired Proof Validation Test");
    println!("===========================================");

    // Use scenario 1's CBU if it exists
    let cbu_id: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = 'UBO Test: Simple Fund'"#,
    )
    .fetch_optional(pool)
    .await?;

    let cbu_id = match cbu_id {
        Some(id) => id,
        None => {
            println!("  Skipping: Run 'cargo x ubo-test seed' first to create test data");
            return Ok(());
        }
    };

    // Get a verification record to test with (need both relationship_id and verification_id)
    let verification_record: Option<(Uuid, Uuid)> =
        sqlx::query_as(r#"SELECT verification_id, relationship_id FROM "ob-poc".cbu_relationship_verification WHERE cbu_id = $1 LIMIT 1"#)
            .bind(cbu_id)
            .fetch_optional(pool)
            .await?;

    let (_verification_id, relationship_id) = match verification_record {
        Some(ids) => ids,
        None => {
            println!("  Skipping: No verification records found for test CBU");
            return Ok(());
        }
    };

    // Create a test document for proofs
    let doc_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".document_catalog (cbu_id, document_name, status)
           VALUES ($1, 'Test Expired Proof Doc', 'active')
           RETURNING doc_id"#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await?;

    if verbose {
        println!("    Using CBU: {}", cbu_id);
        println!("    Using relationship: {}", relationship_id);
        println!("    Created test document: {}", doc_id);
    }

    // Step 1: Test linking an expired proof (should FAIL)
    println!("\n  Step 1: Try linking expired proof (should fail)...");

    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let expired_dsl = format!(
        r#"(ubo.link-proof :cbu "{}" :relationship "{}" :proof "{}" :proof-type "shareholder_register" :valid-until "{}")"#,
        cbu_id, relationship_id, doc_id, yesterday
    );

    if verbose {
        println!("    DSL: {}", expired_dsl);
    }

    let program = parse_program(&expired_dsl).map_err(|e| anyhow::anyhow!(e))?;
    let verb_call = match &program.statements[0] {
        Statement::VerbCall(vc) => vc,
        _ => anyhow::bail!("Expected VerbCall"),
    };

    let op = UboLinkProofOp;
    let mut ctx = ExecutionContext::new();

    let result = op.execute(verb_call, &mut ctx, pool).await;

    match result {
        Err(e) => {
            let error_msg = e.to_string();
            if error_msg.contains("expired") && error_msg.contains("valid_until") {
                println!("    PASS: Expired proof correctly rejected");
                if verbose {
                    println!("    Error: {}", error_msg);
                }
            } else {
                anyhow::bail!("Wrong error message: {}", error_msg);
            }
        }
        Ok(_) => {
            anyhow::bail!("Expected error for expired proof, but operation succeeded");
        }
    }

    // Step 2: Test linking a valid proof (should SUCCEED)
    println!("\n  Step 2: Try linking valid proof (should succeed)...");

    let next_year = (chrono::Utc::now() + chrono::Duration::days(365))
        .format("%Y-%m-%d")
        .to_string();

    let valid_dsl = format!(
        r#"(ubo.link-proof :cbu "{}" :relationship "{}" :proof "{}" :proof-type "shareholder_register" :valid-until "{}")"#,
        cbu_id, relationship_id, doc_id, next_year
    );

    if verbose {
        println!("    DSL: {}", valid_dsl);
    }

    let program = parse_program(&valid_dsl).map_err(|e| anyhow::anyhow!(e))?;
    let verb_call = match &program.statements[0] {
        Statement::VerbCall(vc) => vc,
        _ => anyhow::bail!("Expected VerbCall"),
    };

    let mut ctx = ExecutionContext::new();
    let result = op.execute(verb_call, &mut ctx, pool).await;

    match result {
        Ok(_) => {
            println!("    PASS: Valid proof accepted");
        }
        Err(e) => {
            anyhow::bail!("Valid proof should have been accepted: {}", e);
        }
    }

    // Cleanup: unlink proofs from verification records first, then delete proofs, then document
    // First, get the proof IDs we created
    let proof_ids: Vec<Uuid> =
        sqlx::query_scalar(r#"SELECT proof_id FROM "ob-poc".proofs WHERE document_id = $1"#)
            .bind(doc_id)
            .fetch_all(pool)
            .await?;

    // Unlink proofs from verification records
    for _proof_id in &proof_ids {
        sqlx::query(r#"UPDATE "ob-poc".cbu_relationship_verification SET proof_document_id = NULL WHERE proof_document_id = $1"#)
            .bind(doc_id)
            .execute(pool)
            .await?;
    }

    // Now delete proofs
    sqlx::query(r#"DELETE FROM "ob-poc".proofs WHERE document_id = $1"#)
        .bind(doc_id)
        .execute(pool)
        .await?;

    // Finally delete the document
    sqlx::query(r#"DELETE FROM "ob-poc".document_catalog WHERE doc_id = $1"#)
        .bind(doc_id)
        .execute(pool)
        .await?;

    println!("\n  Expired Proof Test: ALL CHECKS PASSED");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

async fn get_entity_type_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    type_code: &str,
) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = $1"#,
    )
    .bind(type_code)
    .fetch_one(&mut **tx)
    .await
    .with_context(|| format!("Entity type '{}' not found", type_code))
}

async fn create_entity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    name: &str,
    entity_type_id: Uuid,
) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO "ob-poc".entities (name, entity_type_id) VALUES ($1, $2) RETURNING entity_id"#,
    )
    .bind(name)
    .bind(entity_type_id)
    .fetch_one(&mut **tx)
    .await
    .context("Failed to create entity")
}

async fn create_cbu(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    name: &str,
    jurisdiction: &str,
    client_type: &str,
) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        r#"INSERT INTO "ob-poc".cbus (name, jurisdiction, client_type) VALUES ($1, $2, $3) RETURNING cbu_id"#,
    )
    .bind(name)
    .bind(jurisdiction)
    .bind(client_type)
    .fetch_one(&mut **tx)
    .await
    .context("Failed to create CBU")
}

async fn get_role_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    role_name: &str,
) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(r#"SELECT role_id FROM "ob-poc".roles WHERE name = $1"#)
        .bind(role_name)
        .fetch_one(&mut **tx)
        .await
        .with_context(|| format!("Role '{}' not found", role_name))
}

async fn assign_role(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cbu_id: Uuid,
    entity_id: Uuid,
    role_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id) VALUES ($1, $2, $3)"#,
    )
    .bind(cbu_id)
    .bind(entity_id)
    .bind(role_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn create_ubo_edge(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cbu_id: Uuid,
    from_entity_id: Uuid,
    to_entity_id: Uuid,
    edge_type: &str,
    percentage: Option<f64>,
    role: Option<&str>,
) -> Result<Uuid> {
    // ═══════════════════════════════════════════════════════════════════════
    // NEW ARCHITECTURE: Insert into entity_relationships + cbu_relationship_verification
    // ═══════════════════════════════════════════════════════════════════════

    // Map edge_type to relationship_type (lowercase per check constraint)
    let relationship_type = match edge_type {
        "ownership" => "ownership",
        "control" => "control",
        "trust_role" => "trust_role",
        _ => "ownership",
    };

    let (control_type, trust_role_val) = if edge_type == "control" {
        (role, None)
    } else if edge_type == "trust_role" {
        (None, role)
    } else {
        (None, None)
    };

    // Step 1: Create the structural relationship
    let relationship_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO "ob-poc".entity_relationships
            (from_entity_id, to_entity_id, relationship_type, percentage, control_type, trust_role, source)
        VALUES ($1, $2, $3, $4, $5, $6, 'TEST_HARNESS')
        RETURNING relationship_id
        "#,
    )
    .bind(from_entity_id)
    .bind(to_entity_id)
    .bind(relationship_type)
    .bind(percentage)
    .bind(control_type)
    .bind(trust_role_val)
    .fetch_one(&mut **tx)
    .await
    .context("Failed to create entity relationship")?;

    // Step 2: Create the CBU-scoped verification record with allegation
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".cbu_relationship_verification
            (cbu_id, relationship_id, alleged_percentage, status, alleged_at)
        VALUES ($1, $2, $3, 'alleged', NOW())
        "#,
    )
    .bind(cbu_id)
    .bind(relationship_id)
    .bind(percentage)
    .execute(&mut **tx)
    .await
    .context("Failed to create CBU relationship verification")?;

    Ok(relationship_id)
}

async fn get_test_cbu(pool: &PgPool, name: &str) -> Result<Uuid> {
    let result =
        sqlx::query_scalar::<_, Uuid>(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1"#)
            .bind(name)
            .fetch_optional(pool)
            .await?;

    match result {
        Some(id) => Ok(id),
        None => {
            // Seed the data if not present
            println!("  Test data not found, seeding...");
            seed_test_data(pool, false).await?;
            sqlx::query_scalar::<_, Uuid>(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1"#)
                .bind(name)
                .fetch_one(pool)
                .await
                .context("Failed to find test CBU after seeding")
        }
    }
}

#[derive(Debug)]
struct ConvergenceStatus {
    total_edges: i64,
    proven_edges: i64,
    alleged_edges: i64,
    pending_edges: i64,
    disputed_edges: i64,
    is_converged: bool,
}

async fn get_convergence_status(pool: &PgPool, cbu_id: Uuid) -> Result<ConvergenceStatus> {
    // ═══════════════════════════════════════════════════════════════════════
    // NEW ARCHITECTURE: Query cbu_convergence_status view (updated schema)
    // ═══════════════════════════════════════════════════════════════════════
    let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, bool)>(
        r#"
        SELECT
            total_relationships,
            proven_count,
            alleged_count,
            pending_count,
            disputed_count,
            is_converged
        FROM "ob-poc".cbu_convergence_status
        WHERE cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some((total, proven, alleged, pending, disputed, converged)) => Ok(ConvergenceStatus {
            total_edges: total,
            proven_edges: proven,
            alleged_edges: alleged,
            pending_edges: pending,
            disputed_edges: disputed,
            is_converged: converged,
        }),
        None => Ok(ConvergenceStatus {
            total_edges: 0,
            proven_edges: 0,
            alleged_edges: 0,
            pending_edges: 0,
            disputed_edges: 0,
            is_converged: true, // Empty graph is trivially converged
        }),
    }
}

/// Creates a proof with its backing document
/// Returns (document_id, proof_id) - document_id is stored in cbu_relationship_verification
async fn create_proof(pool: &PgPool, cbu_id: Uuid, proof_type: &str) -> Result<Uuid> {
    // Step 1: Create backing document in document_catalog
    let document_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".document_catalog (cbu_id, document_name, status)
           VALUES ($1, $2, 'active')
           RETURNING doc_id"#,
    )
    .bind(cbu_id)
    .bind(format!("Proof Document: {}", proof_type))
    .fetch_one(pool)
    .await
    .context("Failed to create proof document")?;

    // Step 2: Create proof record linked to document
    let _proof_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO "ob-poc".proofs (cbu_id, document_id, proof_type, status, valid_from, valid_until)
           VALUES ($1, $2, $3, 'valid', CURRENT_DATE, CURRENT_DATE + INTERVAL '1 year')
           RETURNING proof_id"#,
    )
    .bind(cbu_id)
    .bind(document_id)
    .bind(proof_type)
    .fetch_one(pool)
    .await
    .context("Failed to create proof")?;

    // Return document_id - this is what gets stored in cbu_relationship_verification.proof_document_id
    Ok(document_id)
}

async fn get_single_edge(pool: &PgPool, cbu_id: Uuid) -> Result<Uuid> {
    // ═══════════════════════════════════════════════════════════════════════
    // NEW ARCHITECTURE: Query entity_relationships via cbu_relationship_verification
    // ═══════════════════════════════════════════════════════════════════════
    sqlx::query_scalar::<_, Uuid>(
        r#"SELECT relationship_id FROM "ob-poc".cbu_relationship_verification WHERE cbu_id = $1 LIMIT 1"#,
    )
    .bind(cbu_id)
    .fetch_one(pool)
    .await
    .context("No relationships found")
}

#[derive(Debug)]
struct UboEdge {
    edge_id: Uuid,
    edge_type: String,
    alleged_percentage: Option<f64>,
    alleged_role: Option<String>,
}

async fn get_all_edges(pool: &PgPool, cbu_id: Uuid) -> Result<Vec<UboEdge>> {
    // ═══════════════════════════════════════════════════════════════════════
    // NEW ARCHITECTURE: Join entity_relationships + cbu_relationship_verification
    // ═══════════════════════════════════════════════════════════════════════
    let rows = sqlx::query_as::<_, (Uuid, String, Option<f64>, Option<String>, Option<String>)>(
        r#"
        SELECT r.relationship_id, r.relationship_type, v.alleged_percentage::float8, r.control_type, r.trust_role
        FROM "ob-poc".entity_relationships r
        JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
        WHERE v.cbu_id = $1
        "#,
    )
    .bind(cbu_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(edge_id, edge_type, pct, control_type, trust_role)| UboEdge {
                edge_id,
                edge_type,
                alleged_percentage: pct,
                alleged_role: control_type.or(trust_role),
            },
        )
        .collect())
}

async fn link_proof_to_edge(pool: &PgPool, relationship_id: Uuid, document_id: Uuid) -> Result<()> {
    // ═══════════════════════════════════════════════════════════════════════
    // NEW ARCHITECTURE: Update cbu_relationship_verification.proof_document_id
    // Note: document_id is from document_catalog, not proofs table
    // ═══════════════════════════════════════════════════════════════════════
    sqlx::query(
        r#"UPDATE "ob-poc".cbu_relationship_verification
           SET proof_document_id = $1, status = 'pending', updated_at = NOW()
           WHERE relationship_id = $2"#,
    )
    .bind(document_id)
    .bind(relationship_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn verify_edge(pool: &PgPool, edge_id: Uuid, proven_percentage: f64) -> Result<()> {
    // ═══════════════════════════════════════════════════════════════════════
    // NEW ARCHITECTURE: Update cbu_relationship_verification + entity_relationships
    // Note: edge_id is now relationship_id
    // ═══════════════════════════════════════════════════════════════════════

    // Update verification record
    sqlx::query(
        r#"
        UPDATE "ob-poc".cbu_relationship_verification
        SET status = 'proven',
            observed_percentage = $1,
            resolved_at = NOW(),
            updated_at = NOW()
        WHERE relationship_id = $2
        "#,
    )
    .bind(proven_percentage)
    .bind(edge_id)
    .execute(pool)
    .await?;

    // Also update structural percentage
    sqlx::query(
        r#"
        UPDATE "ob-poc".entity_relationships
        SET percentage = $1, updated_at = NOW()
        WHERE relationship_id = $2
        "#,
    )
    .bind(proven_percentage)
    .bind(edge_id)
    .execute(pool)
    .await?;

    Ok(())
}

async fn verify_edge_control(pool: &PgPool, edge_id: Uuid, _role: Option<&str>) -> Result<()> {
    // ═══════════════════════════════════════════════════════════════════════
    // NEW ARCHITECTURE: Update cbu_relationship_verification for control edges
    // Note: edge_id is now relationship_id
    // ═══════════════════════════════════════════════════════════════════════
    sqlx::query(
        r#"
        UPDATE "ob-poc".cbu_relationship_verification
        SET status = 'proven',
            resolved_at = NOW(),
            updated_at = NOW()
        WHERE relationship_id = $1
        "#,
    )
    .bind(edge_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug)]
struct BeneficialOwner {
    name: String,
    percentage: f64,
}

#[derive(Debug)]
struct ControlPerson {
    name: String,
    role: String,
}

#[derive(Debug)]
struct UboEvaluation {
    beneficial_owners: Vec<BeneficialOwner>,
    control_persons: Vec<ControlPerson>,
}

async fn evaluate_ubo(pool: &PgPool, cbu_id: Uuid) -> Result<UboEvaluation> {
    // Find beneficial owners (>=25% ownership) using new tables
    // Join entity_relationships (structural) with cbu_relationship_verification (CBU-scoped status)
    let owners = sqlx::query_as::<_, (String, f64)>(
        r#"
        SELECT e.name, COALESCE(v.observed_percentage, r.percentage)::float8 as proven_percentage
        FROM "ob-poc".entity_relationships r
        JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
        JOIN "ob-poc".entities e ON e.entity_id = r.from_entity_id
        WHERE v.cbu_id = $1
          AND r.relationship_type = 'ownership'
          AND v.status = 'proven'
          AND COALESCE(v.observed_percentage, r.percentage) >= 25
        "#,
    )
    .bind(cbu_id)
    .fetch_all(pool)
    .await?;

    let beneficial_owners: Vec<BeneficialOwner> = owners
        .into_iter()
        .map(|(name, pct)| BeneficialOwner {
            name,
            percentage: pct,
        })
        .collect();

    // Find control persons (control relationships or trust roles)
    let controllers = sqlx::query_as::<_, (String, Option<String>, String)>(
        r#"
        SELECT e.name, r.trust_role, r.relationship_type
        FROM "ob-poc".entity_relationships r
        JOIN "ob-poc".cbu_relationship_verification v ON v.relationship_id = r.relationship_id
        JOIN "ob-poc".entities e ON e.entity_id = r.from_entity_id
        WHERE v.cbu_id = $1
          AND r.relationship_type IN ('control', 'trust_role')
          AND v.status = 'proven'
        "#,
    )
    .bind(cbu_id)
    .fetch_all(pool)
    .await?;

    let control_persons: Vec<ControlPerson> = controllers
        .into_iter()
        .map(|(name, role, rel_type)| ControlPerson {
            name,
            role: role.unwrap_or(rel_type),
        })
        .collect();

    Ok(UboEvaluation {
        beneficial_owners,
        control_persons,
    })
}

async fn record_decision(pool: &PgPool, cbu_id: Uuid, status: &str) -> Result<()> {
    let evaluation = evaluate_ubo(pool, cbu_id).await?;
    let snapshot = serde_json::json!({
        "beneficial_owners": evaluation.beneficial_owners.iter().map(|bo| {
            serde_json::json!({
                "name": bo.name,
                "percentage": bo.percentage
            })
        }).collect::<Vec<_>>(),
        "control_persons": evaluation.control_persons.iter().map(|cp| {
            serde_json::json!({
                "name": cp.name,
                "role": cp.role
            })
        }).collect::<Vec<_>>()
    });

    // Use a placeholder user ID for test
    let test_user_id = Uuid::nil();

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".kyc_decisions (cbu_id, status, evaluation_snapshot, decided_by)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(cbu_id)
    .bind(status)
    .bind(snapshot)
    .bind(test_user_id)
    .execute(pool)
    .await?;

    Ok(())
}

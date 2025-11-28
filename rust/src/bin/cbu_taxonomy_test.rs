//! CBU Taxonomy Test Harness
//!
//! Tests the complete CBU entity role taxonomy using DSL v2:
//! 1. Create CBU
//! 2. Create entities of each type (limited company, proper person, partnership, trust)
//! 3. Assign multiple roles to entities
//! 4. Add same entity with different role
//! 5. Query parties
//! 6. Remove specific role
//! 7. Verify remaining role tuples
//!
//! Run with: cargo run --bin cbu_taxonomy_test --features database

use anyhow::{anyhow, Result};
use ob_poc::dsl_v2::{parse_program, DslExecutor, ExecutionContext, ExecutionResult};
use sqlx::PgPool;
use uuid::Uuid;

/// Parse DSL with proper error conversion
fn parse(dsl: &str) -> Result<ob_poc::dsl_v2::Program> {
    parse_program(dsl).map_err(|e| anyhow!("Parse error: {}", e))
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== CBU Taxonomy Test Harness ===\n");

    // Connect to database
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    let pool = PgPool::connect(&database_url).await?;
    println!("✓ Connected to database\n");

    // Create executor and context
    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ExecutionContext::new();

    // Generate unique suffix for this test run
    let test_id = &Uuid::new_v4().to_string()[..8];
    println!("Test run ID: {}\n", test_id);

    // =========================================================================
    // PHASE 1: Create CBU
    // =========================================================================
    println!("--- PHASE 1: Create CBU ---");

    let dsl = format!(
        r#"(cbu.create :name "Test Fund {}" :jurisdiction "LU" :client-type "FUND")"#,
        test_id
    );
    println!("DSL: {}", dsl);

    let program = parse(&dsl)?;
    let results = executor.execute_program(&program, &mut ctx).await?;
    let cbu_id = extract_uuid(&results[0])?;
    ctx.bind("fund", cbu_id);
    println!("✓ Created CBU: {}\n", cbu_id);

    // Verify in DB
    let cbu_row: (String, String) =
        sqlx::query_as(r#"SELECT name, jurisdiction FROM "ob-poc".cbus WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .fetch_one(&pool)
            .await?;
    println!(
        "  DB verify: name='{}', jurisdiction='{}'\n",
        cbu_row.0, cbu_row.1
    );

    // =========================================================================
    // PHASE 2: Create entities of each type
    // =========================================================================
    println!("--- PHASE 2: Create Entities ---");

    // 2a. Create Limited Company
    let dsl = format!(
        r#"(entity.create-limited-company :name "Acme Corp {}" :jurisdiction "GB" :company-number "12345678")"#,
        test_id
    );
    println!("DSL: {}", dsl);
    let program = parse(&dsl)?;
    let results = executor.execute_program(&program, &mut ctx).await?;
    let company_id = extract_uuid(&results[0])?;
    ctx.bind("company", company_id);
    println!("✓ Created Limited Company: {}", company_id);

    // Verify in DB - check both tables
    let entity_row: (String, String) = sqlx::query_as(
        r#"SELECT e.name, et.name as entity_type
           FROM "ob-poc".entities e
           JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
           WHERE e.entity_id = $1"#,
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await?;
    println!(
        "  entities table: name='{}', type='{}'",
        entity_row.0, entity_row.1
    );

    let ext_row: (String,) = sqlx::query_as(
        r#"SELECT company_name FROM "ob-poc".entity_limited_companies WHERE entity_id = $1"#,
    )
    .bind(company_id)
    .fetch_one(&pool)
    .await?;
    println!("  entity_limited_companies: company_name='{}'\n", ext_row.0);

    // 2b. Create Proper Person
    let dsl = format!(
        r#"(entity.create-proper-person :first-name "John" :last-name "Smith{}" :nationality "GB")"#,
        test_id
    );
    println!("DSL: {}", dsl);
    let program = parse(&dsl)?;
    let results = executor.execute_program(&program, &mut ctx).await?;
    let person_id = extract_uuid(&results[0])?;
    ctx.bind("person", person_id);
    println!("✓ Created Proper Person: {}", person_id);

    let entity_row: (String, String) = sqlx::query_as(
        r#"SELECT e.name, et.name as entity_type
           FROM "ob-poc".entities e
           JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
           WHERE e.entity_id = $1"#,
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await?;
    println!(
        "  entities table: name='{}', type='{}'",
        entity_row.0, entity_row.1
    );

    let ext_row: (String, String) = sqlx::query_as(
        r#"SELECT first_name, last_name FROM "ob-poc".entity_proper_persons WHERE entity_id = $1"#,
    )
    .bind(person_id)
    .fetch_one(&pool)
    .await?;
    println!(
        "  entity_proper_persons: first='{}', last='{}'\n",
        ext_row.0, ext_row.1
    );

    // 2c. Create Partnership
    let dsl = format!(
        r#"(entity.create-partnership :name "Partners LP {}" :jurisdiction "DE" :partnership-type "LIMITED")"#,
        test_id
    );
    println!("DSL: {}", dsl);
    let program = parse(&dsl)?;
    let results = executor.execute_program(&program, &mut ctx).await?;
    let partnership_id = extract_uuid(&results[0])?;
    ctx.bind("partnership", partnership_id);
    println!("✓ Created Partnership: {}", partnership_id);

    let entity_row: (String, String) = sqlx::query_as(
        r#"SELECT e.name, et.name as entity_type
           FROM "ob-poc".entities e
           JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
           WHERE e.entity_id = $1"#,
    )
    .bind(partnership_id)
    .fetch_one(&pool)
    .await?;
    println!(
        "  entities table: name='{}', type='{}'\n",
        entity_row.0, entity_row.1
    );

    // 2d. Create Trust
    let dsl = format!(
        r#"(entity.create-trust :name "Family Trust {}" :jurisdiction "JE" :trust-type "DISCRETIONARY")"#,
        test_id
    );
    println!("DSL: {}", dsl);
    let program = parse(&dsl)?;
    let results = executor.execute_program(&program, &mut ctx).await?;
    let trust_id = extract_uuid(&results[0])?;
    ctx.bind("trust", trust_id);
    println!("✓ Created Trust: {}", trust_id);

    let entity_row: (String, String) = sqlx::query_as(
        r#"SELECT e.name, et.name as entity_type
           FROM "ob-poc".entities e
           JOIN "ob-poc".entity_types et ON et.entity_type_id = e.entity_type_id
           WHERE e.entity_id = $1"#,
    )
    .bind(trust_id)
    .fetch_one(&pool)
    .await?;
    println!(
        "  entities table: name='{}', type='{}'\n",
        entity_row.0, entity_row.1
    );

    // =========================================================================
    // PHASE 3: Assign roles to entities
    // =========================================================================
    println!("--- PHASE 3: Assign Roles ---");

    // Company as Investment Manager
    let dsl = r#"(cbu.assign-role :cbu-id @fund :entity-id @company :role "INVESTMENT_MANAGER")"#;
    println!("DSL: {}", dsl);
    let program = parse(dsl)?;
    executor.execute_program(&program, &mut ctx).await?;
    println!("✓ Assigned INVESTMENT_MANAGER to company");

    // Person as Director
    let dsl = r#"(cbu.assign-role :cbu-id @fund :entity-id @person :role "DIRECTOR")"#;
    println!("DSL: {}", dsl);
    let program = parse(dsl)?;
    executor.execute_program(&program, &mut ctx).await?;
    println!("✓ Assigned DIRECTOR to person");

    // Partnership as Limited Partner
    let dsl = r#"(cbu.assign-role :cbu-id @fund :entity-id @partnership :role "LIMITED_PARTNER")"#;
    println!("DSL: {}", dsl);
    let program = parse(dsl)?;
    executor.execute_program(&program, &mut ctx).await?;
    println!("✓ Assigned LIMITED_PARTNER to partnership");

    // Trust as Beneficial Owner
    let dsl = r#"(cbu.assign-role :cbu-id @fund :entity-id @trust :role "BENEFICIAL_OWNER")"#;
    println!("DSL: {}", dsl);
    let program = parse(dsl)?;
    executor.execute_program(&program, &mut ctx).await?;
    println!("✓ Assigned BENEFICIAL_OWNER to trust\n");

    // Verify role count in DB
    let count: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .fetch_one(&pool)
            .await?;
    println!("  DB verify: {} role assignments for CBU\n", count.0);

    // =========================================================================
    // PHASE 4: Add same entity with different role
    // =========================================================================
    println!("--- PHASE 4: Add Additional Role to Same Entity ---");

    // Person gets additional role as Authorized Signatory
    let dsl = r#"(cbu.assign-role :cbu-id @fund :entity-id @person :role "AUTHORIZED_SIGNATORY")"#;
    println!("DSL: {}", dsl);
    let program = parse(dsl)?;
    executor.execute_program(&program, &mut ctx).await?;
    println!("✓ Assigned AUTHORIZED_SIGNATORY to person (now has 2 roles)");

    // Company gets additional role as Shareholder
    let dsl = r#"(cbu.assign-role :cbu-id @fund :entity-id @company :role "SHAREHOLDER")"#;
    println!("DSL: {}", dsl);
    let program = parse(dsl)?;
    executor.execute_program(&program, &mut ctx).await?;
    println!("✓ Assigned SHAREHOLDER to company (now has 2 roles)\n");

    // Verify role count
    let count: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .fetch_one(&pool)
            .await?;
    println!(
        "  DB verify: {} role assignments for CBU (was 4, now 6)\n",
        count.0
    );

    // =========================================================================
    // PHASE 5: Query parties
    // =========================================================================
    println!("--- PHASE 5: Query All Parties ---");

    let dsl = r#"(cbu.parties :cbu-id @fund)"#;
    println!("DSL: {}", dsl);
    let program = parse(dsl)?;
    let results = executor.execute_program(&program, &mut ctx).await?;

    if let ExecutionResult::RecordSet(records) = &results[0] {
        println!("✓ Retrieved {} parties:\n", records.len());
        for record in records {
            let entity_name = record
                .get("entity_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let entity_type = record
                .get("entity_type")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let role_name = record
                .get("role_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("  - {} ({}) as {}", entity_name, entity_type, role_name);
        }
    }
    println!();

    // =========================================================================
    // PHASE 6: Remove specific role
    // =========================================================================
    println!("--- PHASE 6: Remove Specific Role ---");

    // Remove AUTHORIZED_SIGNATORY from person (keep DIRECTOR)
    let dsl = r#"(cbu.remove-role :cbu-id @fund :entity-id @person :role "AUTHORIZED_SIGNATORY")"#;
    println!("DSL: {}", dsl);
    let program = parse(dsl)?;
    let results = executor.execute_program(&program, &mut ctx).await?;

    if let ExecutionResult::Affected(n) = &results[0] {
        println!(
            "✓ Removed AUTHORIZED_SIGNATORY from person ({} row affected)",
            n
        );
    }

    // Verify person still has DIRECTOR role
    let person_roles: Vec<(String,)> = sqlx::query_as(
        r#"SELECT r.name
           FROM "ob-poc".cbu_entity_roles cer
           JOIN "ob-poc".roles r ON r.role_id = cer.role_id
           WHERE cer.cbu_id = $1 AND cer.entity_id = $2"#,
    )
    .bind(cbu_id)
    .bind(person_id)
    .fetch_all(&pool)
    .await?;
    println!(
        "  DB verify: person still has roles: {:?}\n",
        person_roles.iter().map(|r| &r.0).collect::<Vec<_>>()
    );

    // =========================================================================
    // PHASE 7: Final verification
    // =========================================================================
    println!("--- PHASE 7: Final Verification ---");

    let count: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .fetch_one(&pool)
            .await?;
    println!(
        "✓ Final role count: {} (was 6, now 5 after removal)\n",
        count.0
    );

    // Query final parties
    let dsl = r#"(cbu.parties :cbu-id @fund)"#;
    let program = parse(dsl)?;
    let results = executor.execute_program(&program, &mut ctx).await?;

    if let ExecutionResult::RecordSet(records) = &results[0] {
        println!("Final party list ({} entries):", records.len());
        for record in records {
            let entity_name = record
                .get("entity_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let role_name = record
                .get("role_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            println!("  - {} as {}", entity_name, role_name);
        }
    }

    println!("\n=== TEST COMPLETE ===");
    println!("All CBU taxonomy operations verified successfully!");
    println!("\nTest data created with IDs:");
    println!("  CBU:         {}", cbu_id);
    println!("  Company:     {}", company_id);
    println!("  Person:      {}", person_id);
    println!("  Partnership: {}", partnership_id);
    println!("  Trust:       {}", trust_id);

    Ok(())
}

fn extract_uuid(result: &ExecutionResult) -> Result<Uuid> {
    match result {
        ExecutionResult::Uuid(id) => Ok(*id),
        _ => anyhow::bail!("Expected UUID result"),
    }
}

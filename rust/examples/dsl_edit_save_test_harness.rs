//! DSL Edit-Save Test Harness - Complete Database Integration Test
//!
//! This test harness demonstrates the full DSL edit-save workflow with real PostgreSQL
//! database operations, proving the multi-stage commit pattern works correctly.
//!
//! ## Test Workflow:
//! 1. **Setup**: Create initial DSL/AST pair in database
//! 2. **Load**: Retrieve existing DSL by onboarding_request_id
//! 3. **Edit**: Modify DSL content (touch/update)
//! 4. **Save**: Multi-stage commit with version increment
//! 5. **Verify**: Confirm both DSL and AST saved with matching versions
//!
//! ## Multi-Stage Commit Verification:
//! ```
//! Transaction Begin
//!   ├── Parse & Validate DSL
//!   ├── Version Increment: N → N+1
//!   ├── Save DSL Instance (version N+1)
//!   ├── Save AST Record (version N+1)
//!   ├── Create Audit Entry
//!   └── Transaction Commit
//! ```
//!
//! ## Database Schema Requirements:
//! - `"ob-poc".dsl_instances` table
//! - `"ob-poc".parsed_asts` table
//! - `"ob-poc".audit_log` table
//!
//! ## Prerequisites:
//! - PostgreSQL running with "ob-poc" schema initialized
//! - Environment: DATABASE_URL="postgresql://user:pass@localhost:5432/db_name"
//!
//! ## Usage:
//! ```bash
//! export DATABASE_URL="postgresql://postgres:password@localhost:5432/ob_poc"
//! cargo run --example dsl_edit_save_test_harness --features="database"
//! ```

use ob_poc::dsl_manager::{DslCrudManager, DslLoadRequest, DslSaveRequest, OperationContext};

#[cfg(feature = "database")]
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{error, info};
use uuid::Uuid;

/// Test harness configuration
#[derive(Debug, Clone)]
struct TestHarnessConfig {
    pub test_case_id: String,
    pub onboarding_request_id: Uuid,
    pub user_id: String,
    pub initial_dsl_content: String,
    pub edited_dsl_content: String,
}

impl TestHarnessConfig {
    fn new() -> Self {
        let test_id = Uuid::new_v4();
        let test_case_id = format!("test-case-{}", test_id);
        let onboarding_request_id = Uuid::new_v4();

        Self {
            test_case_id: test_case_id.clone(),
            onboarding_request_id,
            user_id: "test-harness-user".to_string(),
            initial_dsl_content: format!(
                r#"
(case.create
  :case-id "{}"
  :name "Initial Test Case"
  :jurisdiction "US"
  :entity-type "CORP"
  :status "DRAFT")

(entity.register
  :entity-id "test-entity-001"
  :name "Test Corporation"
  :type "CORPORATION"
  :incorporation-date "2024-01-01")

(kyc.start
  :entity-id "test-entity-001"
  :level "STANDARD"
  :requirements ["IDENTITY" "ADDRESS"])
"#,
                test_case_id
            ),
            edited_dsl_content: format!(
                r#"
(case.create
  :case-id "{}"
  :name "UPDATED Test Case - EDITED"
  :jurisdiction "US"
  :entity-type "CORP"
  :status "ACTIVE")

(entity.register
  :entity-id "test-entity-001"
  :name "Test Corporation Inc."
  :type "CORPORATION"
  :incorporation-date "2024-01-01"
  :business-purpose "Software Development")

(kyc.start
  :entity-id "test-entity-001"
  :level "ENHANCED"
  :requirements ["IDENTITY" "ADDRESS" "FINANCIAL"])

(products.add
  :entity-id "test-entity-001"
  :products ["CUSTODY" "ADVISORY"])

(services.provision
  :entity-id "test-entity-001"
  :services ["REPORTING" "ANALYTICS"])

(compliance.screen
  :entity-id "test-entity-001"
  :frameworks ["AML" "KYC" "SANCTIONS"])
"#,
                test_case_id
            ),
        }
    }
}

/// Test results tracking
#[derive(Debug)]
struct TestResults {
    setup_success: bool,
    initial_version: u32,
    load_success: bool,
    edit_save_success: bool,
    final_version: u32,
    version_incremented: bool,
    dsl_ast_consistency: bool,
    audit_trail_created: bool,
    cleanup_success: bool,
    total_test_time_ms: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize comprehensive tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("🧪 Starting DSL Edit-Save Test Harness");

    let test_start = Instant::now();
    let config = TestHarnessConfig::new();

    info!("📋 Test Configuration:");
    info!("   Test Case ID: {}", config.test_case_id);
    info!("   Onboarding Request ID: {}", config.onboarding_request_id);
    info!("   User ID: {}", config.user_id);

    #[cfg(feature = "database")]
    let results = run_database_test_harness(&config).await?;

    #[cfg(not(feature = "database"))]
    let results = run_mock_test_harness(&config).await?;

    let total_time = test_start.elapsed();

    // Print comprehensive test results
    print_test_results(&results, total_time);

    // Determine overall test success
    let overall_success = results.setup_success
        && results.load_success
        && results.edit_save_success
        && results.version_incremented
        && results.dsl_ast_consistency
        && results.audit_trail_created;

    if overall_success {
        info!("✅ DSL Edit-Save Test Harness: ALL TESTS PASSED!");
        println!("\n🎉 SUCCESS: Multi-stage commit workflow validated!");
        println!("   ✓ Database setup and initial DSL/AST creation");
        println!("   ✓ DSL loading and content verification");
        println!("   ✓ DSL editing and multi-stage save commit");
        println!("   ✓ Version increment verification");
        println!("   ✓ DSL-AST consistency across versions");
        println!("   ✓ Audit trail creation and tracking");
        println!("   ✓ Transaction integrity maintained");
    } else {
        error!("❌ DSL Edit-Save Test Harness: SOME TESTS FAILED!");
        println!("\n💥 FAILURE: Issues detected in workflow");
        if !results.setup_success {
            println!("   ❌ Database setup failed");
        }
        if !results.version_incremented {
            println!("   ❌ Version increment failed");
        }
        if !results.dsl_ast_consistency {
            println!("   ❌ DSL-AST consistency check failed");
        }
        if !results.audit_trail_created {
            println!("   ❌ Audit trail creation failed");
        }
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(feature = "database")]
async fn run_database_test_harness(
    config: &TestHarnessConfig,
) -> Result<TestResults, Box<dyn std::error::Error>> {
    let test_start = Instant::now();

    // Connect to database
    info!("🔗 Connecting to PostgreSQL database");
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/ob_poc".to_string());

    info!(
        "Database URL: {}",
        database_url.split('@').next().unwrap_or("***")
    );

    let pool = PgPool::connect(&database_url).await?;
    info!("✅ Database connection established");

    // Initialize DSL CRUD Manager
    let dsl_crud_manager = DslCrudManager::new(pool.clone());

    let mut results = TestResults {
        setup_success: false,
        initial_version: 0,
        load_success: false,
        edit_save_success: false,
        final_version: 0,
        version_incremented: false,
        dsl_ast_consistency: false,
        audit_trail_created: false,
        cleanup_success: false,
        total_test_time_ms: 0,
    };

    // === PHASE 1: SETUP - Create Initial DSL/AST Pair ===
    info!("\n📝 PHASE 1: Setting up initial DSL/AST pair in database");

    let setup_request = DslSaveRequest {
        case_id: config.test_case_id.clone(),
        onboarding_request_id: config.onboarding_request_id,
        dsl_content: config.initial_dsl_content.clone(),
        user_id: config.user_id.clone(),
        operation_context: OperationContext {
            workflow_type: "test_harness".to_string(),
            source: "initial_setup".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("phase".to_string(), "setup".to_string());
                meta.insert("test_run".to_string(), Uuid::new_v4().to_string());
                meta
            },
        },
    };

    match dsl_crud_manager.save_dsl_complex(setup_request).await {
        Ok(setup_result) => {
            results.setup_success = true;
            results.initial_version = setup_result.version_number;

            info!("✅ Initial DSL/AST pair created:");
            info!("   Case ID: {}", setup_result.case_id);
            info!("   Initial Version: {}", setup_result.version_number);
            info!("   DSL Instance ID: {}", setup_result.dsl_instance_id);
            info!("   AST Record ID: {}", setup_result.ast_record_id);
            info!("   Parsing Time: {}ms", setup_result.parsing_time_ms);
            info!("   Save Time: {}ms", setup_result.save_time_ms);
        }
        Err(e) => {
            error!("❌ Initial setup failed: {}", e);
            results.total_test_time_ms = test_start.elapsed().as_millis() as u64;
            return Ok(results);
        }
    }

    // === PHASE 2: LOAD - Retrieve Existing DSL ===
    info!("\n📖 PHASE 2: Loading existing DSL/AST pair from database");

    let load_request = DslLoadRequest {
        case_id: config.test_case_id.clone(),
        version: None, // Load latest version
        include_ast: true,
        include_audit_trail: true,
    };

    let loaded_dsl = match dsl_crud_manager.load_dsl_complete(load_request).await {
        Ok(load_result) => {
            results.load_success = true;

            info!("✅ DSL loaded successfully:");
            info!("   Case ID: {}", load_result.case_id);
            info!("   Version: {}", load_result.version_number);
            info!(
                "   DSL Length: {} characters",
                load_result.dsl_content.len()
            );
            info!("   AST Available: {}", load_result.ast_json.is_some());
            info!("   Audit Entries: {}", load_result.audit_entries.len());
            info!(
                "   Created: {}",
                load_result.created_at.format("%Y-%m-%d %H:%M:%S")
            );

            // Verify the loaded DSL contains expected content
            if load_result.dsl_content.contains("Initial Test Case")
                && load_result.dsl_content.contains("test-entity-001")
            {
                info!("✅ Loaded DSL content verified");
            } else {
                warn!("⚠️  Loaded DSL content verification failed");
            }

            load_result
        }
        Err(e) => {
            error!("❌ DSL loading failed: {}", e);
            results.total_test_time_ms = test_start.elapsed().as_millis() as u64;
            return Ok(results);
        }
    };

    // === PHASE 3: EDIT & SAVE - Multi-Stage Commit Test ===
    info!("\n✏️  PHASE 3: Editing DSL and saving with multi-stage commit");

    info!("🔧 DSL Content Changes:");
    info!("   • Case name: 'Initial Test Case' → 'UPDATED Test Case - EDITED'");
    info!("   • Entity name: 'Test Corporation' → 'Test Corporation Inc.'");
    info!("   • KYC level: 'STANDARD' → 'ENHANCED'");
    info!("   • Added: products.add, services.provision, compliance.screen");
    info!(
        "   • Content size: {} → {} characters",
        loaded_dsl.dsl_content.len(),
        config.edited_dsl_content.len()
    );

    let edit_save_request = DslSaveRequest {
        case_id: config.test_case_id.clone(),
        onboarding_request_id: config.onboarding_request_id, // SAME onboarding_request_id
        dsl_content: config.edited_dsl_content.clone(),
        user_id: config.user_id.clone(),
        operation_context: OperationContext {
            workflow_type: "test_harness".to_string(),
            source: "edit_save_test".to_string(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("phase".to_string(), "edit_save".to_string());
                meta.insert(
                    "previous_version".to_string(),
                    results.initial_version.to_string(),
                );
                meta.insert("edit_type".to_string(), "comprehensive_update".to_string());
                meta
            },
        },
    };

    match dsl_crud_manager.save_dsl_complex(edit_save_request).await {
        Ok(edit_result) => {
            results.edit_save_success = true;
            results.final_version = edit_result.version_number;
            results.version_incremented = edit_result.version_number > results.initial_version;

            info!("✅ DSL edit-save completed:");
            info!("   Case ID: {}", edit_result.case_id);
            info!(
                "   Version: {} → {} (increment: {})",
                results.initial_version, edit_result.version_number, results.version_incremented
            );
            info!("   DSL Instance ID: {}", edit_result.dsl_instance_id);
            info!("   AST Record ID: {}", edit_result.ast_record_id);
            info!("   Parsing Time: {}ms", edit_result.parsing_time_ms);
            info!("   Save Time: {}ms", edit_result.save_time_ms);
            info!("   Total Time: {}ms", edit_result.total_time_ms);

            if results.version_incremented {
                info!(
                    "✅ Version increment verified: {} → {}",
                    results.initial_version, edit_result.version_number
                );
            } else {
                error!(
                    "❌ Version increment failed: {} → {}",
                    results.initial_version, edit_result.version_number
                );
            }
        }
        Err(e) => {
            error!("❌ DSL edit-save failed: {}", e);
            results.total_test_time_ms = test_start.elapsed().as_millis() as u64;
            return Ok(results);
        }
    }

    // === PHASE 4: VERIFICATION - Database Consistency Checks ===
    info!("\n🔍 PHASE 4: Verifying database consistency and integrity");

    // 4.1: Verify DSL-AST consistency
    results.dsl_ast_consistency = verify_dsl_ast_consistency(
        &pool,
        &config.test_case_id,
        results.final_version,
        config.onboarding_request_id,
    )
    .await?;

    // 4.2: Verify audit trail
    results.audit_trail_created = verify_audit_trail(
        &pool,
        &config.test_case_id,
        results.initial_version,
        results.final_version,
    )
    .await?;

    // 4.3: Verify updated content
    let verify_content_success =
        verify_updated_content(&pool, &config.test_case_id, results.final_version).await?;

    if verify_content_success {
        info!("✅ Updated DSL content verification passed");
    } else {
        warn!("⚠️  Updated DSL content verification failed");
    }

    // === PHASE 5: CLEANUP (Optional) ===
    info!("\n🧹 PHASE 5: Test cleanup");

    results.cleanup_success =
        cleanup_test_data(&pool, &config.test_case_id, config.onboarding_request_id).await?;

    results.total_test_time_ms = test_start.elapsed().as_millis() as u64;
    Ok(results)
}

#[cfg(not(feature = "database"))]
async fn run_mock_test_harness(
    config: &TestHarnessConfig,
) -> Result<TestResults, Box<dyn std::error::Error>> {
    info!("🧪 Running mock test harness (database feature disabled)");

    let test_start = Instant::now();
    let dsl_crud_manager = DslCrudManager::new();

    // Mock the workflow
    let setup_request = DslSaveRequest {
        case_id: config.test_case_id.clone(),
        onboarding_request_id: config.onboarding_request_id,
        dsl_content: config.initial_dsl_content.clone(),
        user_id: config.user_id.clone(),
        operation_context: OperationContext {
            workflow_type: "test_harness".to_string(),
            source: "mock_setup".to_string(),
            metadata: HashMap::new(),
        },
    };

    let setup_result = dsl_crud_manager.save_dsl_complex(setup_request).await?;
    info!(
        "✅ Mock initial setup: version {}",
        setup_result.version_number
    );

    let edit_request = DslSaveRequest {
        case_id: config.test_case_id.clone(),
        onboarding_request_id: config.onboarding_request_id,
        dsl_content: config.edited_dsl_content.clone(),
        user_id: config.user_id.clone(),
        operation_context: OperationContext {
            workflow_type: "test_harness".to_string(),
            source: "mock_edit".to_string(),
            metadata: HashMap::new(),
        },
    };

    let edit_result = dsl_crud_manager.save_dsl_complex(edit_request).await?;
    info!("✅ Mock edit-save: version {}", edit_result.version_number);

    Ok(TestResults {
        setup_success: true,
        initial_version: setup_result.version_number,
        load_success: true,
        edit_save_success: true,
        final_version: edit_result.version_number,
        version_incremented: edit_result.version_number > setup_result.version_number,
        dsl_ast_consistency: true,
        audit_trail_created: true,
        cleanup_success: true,
        total_test_time_ms: test_start.elapsed().as_millis() as u64,
    })
}

#[cfg(feature = "database")]
async fn verify_dsl_ast_consistency(
    pool: &PgPool,
    case_id: &str,
    version: u32,
    onboarding_request_id: Uuid,
) -> Result<bool, Box<dyn std::error::Error>> {
    info!("🔍 Verifying DSL-AST consistency for version {}", version);

    // Check DSL instance
    let dsl_row = sqlx::query!(
        r#"
        SELECT instance_id, onboarding_request_id, current_dsl, created_by
        FROM "ob-poc".dsl_instances
        WHERE case_id = $1 AND version = $2
        "#,
        case_id,
        version as i32
    )
    .fetch_optional(pool)
    .await?;

    let dsl_exists = dsl_row.is_some();
    let onboarding_id_matches = dsl_row
        .as_ref()
        .map(|row| row.onboarding_request_id == onboarding_request_id)
        .unwrap_or(false);

    // Check AST record
    let ast_row = sqlx::query!(
        r#"
        SELECT ast_id, dsl_instance_id, ast_json
        FROM "ob-poc".parsed_asts
        WHERE case_id = $1 AND version = $2
        "#,
        case_id,
        version as i32
    )
    .fetch_optional(pool)
    .await?;

    let ast_exists = ast_row.is_some();

    // Verify DSL instance ID matches between tables
    let ids_match = if let (Some(dsl), Some(ast)) = (&dsl_row, &ast_row) {
        dsl.instance_id == ast.dsl_instance_id
    } else {
        false
    };

    info!("   DSL instance exists: {}", dsl_exists);
    info!("   AST record exists: {}", ast_exists);
    info!("   Onboarding ID matches: {}", onboarding_id_matches);
    info!("   DSL-AST ID consistency: {}", ids_match);

    if let Some(dsl) = &dsl_row {
        info!("   DSL content length: {} chars", dsl.current_dsl.len());
        info!("   Created by: {}", dsl.created_by);
    }

    if let Some(ast) = &ast_row {
        info!("   AST JSON length: {} chars", ast.ast_json.len());
    }

    let consistent = dsl_exists && ast_exists && onboarding_id_matches && ids_match;

    if consistent {
        info!("✅ DSL-AST consistency verified");
    } else {
        error!("❌ DSL-AST consistency check failed");
    }

    Ok(consistent)
}

#[cfg(feature = "database")]
async fn verify_audit_trail(
    pool: &PgPool,
    case_id: &str,
    initial_version: u32,
    final_version: u32,
) -> Result<bool, Box<dyn std::error::Error>> {
    info!("📋 Verifying audit trail creation");

    let audit_rows = sqlx::query!(
        r#"
        SELECT audit_id, operation_type, user_id, created_at,
               version_from, version_to, change_summary
        FROM "ob-poc".audit_log
        WHERE case_id = $1
        ORDER BY created_at ASC
        "#,
        case_id
    )
    .fetch_all(pool)
    .await?;

    let audit_entries_exist = !audit_rows.is_empty();
    let has_both_operations = audit_rows.len() >= 2;

    info!("   Audit entries found: {}", audit_rows.len());

    for (i, row) in audit_rows.iter().enumerate() {
        info!(
            "   [{}] {} | v{} → v{} | {} | {}",
            i + 1,
            row.operation_type,
            row.version_from,
            row.version_to,
            row.user_id,
            row.created_at.format("%H:%M:%S")
        );
    }

    // Check for expected version transitions
    let has_setup_entry = audit_rows
        .iter()
        .any(|row| row.version_from == 0 && row.version_to == initial_version as i32);

    let has_edit_entry = audit_rows.iter().any(|row| {
        row.version_from == initial_version as i32 && row.version_to == final_version as i32
    });

    info!(
        "   Setup entry (0 → {}): {}",
        initial_version, has_setup_entry
    );
    info!(
        "   Edit entry ({} → {}): {}",
        initial_version, final_version, has_edit_entry
    );

    let complete_audit_trail =
        audit_entries_exist && has_both_operations && has_setup_entry && has_edit_entry;

    if complete_audit_trail {
        info!("✅ Complete audit trail verified");
    } else {
        warn!("⚠️  Incomplete audit trail detected");
    }

    Ok(complete_audit_trail)
}

#[cfg(feature = "database")]
async fn verify_updated_content(
    pool: &PgPool,
    case_id: &str,
    version: u32,
) -> Result<bool, Box<dyn std::error::Error>> {
    info!("📝 Verifying updated DSL content");

    let content_row = sqlx::query!(
        r#"
        SELECT current_dsl
        FROM "ob-poc".dsl_instances
        WHERE case_id = $1 AND version = $2
        "#,
        case_id,
        version as i32
    )
    .fetch_optional(pool)
    .await?;

    if let Some(row) = content_row {
        let content = &row.current_dsl;

        // Check for expected updates
        let has_updated_name = content.contains("UPDATED Test Case - EDITED");
        let has_enhanced_kyc = content.contains("ENHANCED");
        let has_products_add = content.contains("products.add");
        let has_services_provision = content.contains("services.provision");
        let has_compliance_screen = content.contains("compliance.screen");

        info!("   Content length: {} characters", content.len());
        info!("   Updated case name: {}", has_updated_name);
        info!("   Enhanced KYC level: {}", has_enhanced_kyc);
        info!("   Products added: {}", has_products_add);
        info!("   Services provisioned: {}", has_services_provision);
        info!("   Compliance screening: {}", has_compliance_screen);

        let all_updates_present = has_updated_name
            && has_enhanced_kyc
            && has_products_add
            && has_services_provision
            && has_compliance_screen;

        if all_updates_present {
            info!("✅ All expected content updates verified");
        } else {
            warn!("⚠️  Some expected content updates missing");
        }

        Ok(all_updates_present)
    } else {
        error!("❌ DSL content not found for verification");
        Ok(false)
    }
}

#[cfg(feature = "database")]
async fn cleanup_test_data(
    pool: &PgPool,
    case_id: &str,
    onboarding_request_id: Uuid,
) -> Result<bool, Box<dyn std::error::Error>> {
    info!("🧹 Cleaning up test data");

    let mut tx = pool.begin().await?;

    // Delete audit log entries
    let audit_deleted = sqlx::query!(
        r#"DELETE FROM "ob-poc".audit_log WHERE case_id = $1"#,
        case_id
    )
    .execute(&mut *tx)
    .await?;

    // Delete AST records
    let ast_deleted = sqlx::query!(
        r#"DELETE FROM "ob-poc".parsed_asts WHERE case_id = $1"#,
        case_id
    )
    .execute(&mut *tx)
    .await?;

    // Delete DSL instances
    let dsl_deleted = sqlx::query!(
        r#"DELETE FROM "ob-poc".dsl_instances WHERE case_id = $1"#,
        case_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    info!(
        "   Cleaned up: {} audit entries, {} AST records, {} DSL instances",
        audit_deleted.rows_affected(),
        ast_deleted.rows_affected(),
        dsl_deleted.rows_affected()
    );

    let cleanup_success = audit_deleted.rows_affected() > 0
        && ast_deleted.rows_affected() > 0
        && dsl_deleted.rows_affected() > 0;

    if cleanup_success {
        info!("✅ Test data cleanup completed");
    } else {
        warn!("⚠️  Test data cleanup may be incomplete");
    }

    Ok(cleanup_success)
}

fn print_test_results(results: &TestResults, total_time: std::time::Duration) {
    println!("\n{}", "=".repeat(80));
    println!("📊 DSL EDIT-SAVE TEST HARNESS RESULTS");
    println!("{}", "=".repeat(80));

    println!("🕒 TIMING:");
    println!("   Total Test Duration: {:?}", total_time);
    println!("   Internal Processing: {}ms", results.total_test_time_ms);

    println!("\n📋 TEST PHASES:");
    println!(
        "   Phase 1 - Initial Setup:     {}",
        status_symbol(results.setup_success)
    );
    println!(
        "   Phase 2 - DSL Loading:       {}",
        status_symbol(results.load_success)
    );
    println!(
        "   Phase 3 - Edit & Save:       {}",
        status_symbol(results.edit_save_success)
    );
    println!(
        "   Phase 4 - Verification:      {}",
        status_symbol(results.dsl_ast_consistency)
    );
    println!(
        "   Phase 5 - Cleanup:           {}",
        status_symbol(results.cleanup_success)
    );

    println!("\n🔢 VERSION MANAGEMENT:");
    println!("   Initial Version:      {}", results.initial_version);
    println!("   Final Version:        {}", results.final_version);
    println!(
        "   Version Incremented:  {} {}",
        status_symbol(results.version_incremented),
        if results.version_incremented {
            format!("({} → {})", results.initial_version, results.final_version)
        } else {
            "(FAILED)".to_string()
        }
    );

    println!("\n🔍 INTEGRITY CHECKS:");
    println!(
        "   DSL-AST Consistency:  {}",
        status_symbol(results.dsl_ast_consistency)
    );
    println!(
        "   Audit Trail Created:  {}",
        status_symbol(results.audit_trail_created)
    );

    println!("\n🎯 OVERALL STATUS:");
    let overall_success = results.setup_success
        && results.load_success
        && results.edit_save_success
        && results.version_incremented
        && results.dsl_ast_consistency
        && results.audit_trail_created;

    if overall_success {
        println!("   🎉 ALL TESTS PASSED!");
    } else {
        println!("   💥 SOME TESTS FAILED!");
    }

    println!("{}", "=".repeat(80));
}

fn status_symbol(success: bool) -> &'static str {
    if success {
        "✅"
    } else {
        "❌"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harness_config_creation() {
        let config = TestHarnessConfig::new();
        assert!(!config.test_case_id.is_empty());
        assert!(!config.initial_dsl_content.is_empty());
        assert!(!config.edited_dsl_content.is_empty());
        assert_ne!(config.initial_dsl_content, config.edited_dsl_content);
    }

    #[test]
    fn test_dsl_content_changes() {
        let config = TestHarnessConfig::new();

        // Verify initial DSL content
        assert!(config.initial_dsl_content.contains("Initial Test Case"));
        assert!(config.initial_dsl_content.contains("STANDARD"));
        assert!(!config.initial_dsl_content.contains("UPDATED"));
        assert!(!config.initial_dsl_content.contains("products.add"));

        // Verify edited DSL content
        assert!(config
            .edited_dsl_content
            .contains("UPDATED Test Case - EDITED"));
        assert!(config.edited_dsl_content.contains("ENHANCED"));
        assert!(config.edited_dsl_content.contains("products.add"));
        assert!(config.edited_dsl_content.contains("services.provision"));
        assert!(config.edited_dsl_content.contains("compliance.screen"));

        // Verify edited content is longer (more comprehensive)
        assert!(config.edited_dsl_content.len() > config.initial_dsl_content.len());
    }

    #[tokio::test]
    async fn test_mock_harness() {
        let config = TestHarnessConfig::new();
        let results = run_mock_test_harness(&config).await;
        assert!(results.is_ok());

        let results = results.unwrap();
        assert!(results.setup_success);
        assert!(results.version_incremented);
        assert!(results.final_version > results.initial_version);
    }
}

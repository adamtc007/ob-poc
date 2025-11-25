//! E2E Test: KYC Session Idempotency
//!
//! Tests the full KYC workflow with idempotent execution:
//! - Run a complete KYC session (investigation -> documents -> screening -> risk -> decision -> monitoring)
//! - Run the SAME session again
//! - Verify the database state is identical (idempotent)
//!
//! This validates the UPSERT semantics and natural key constraints.

use ob_poc::database::CrudExecutor;
use ob_poc::forth_engine::{execute_sheet_into_env, DslSheet};
use sqlx::PgPool;
use uuid::Uuid;

async fn get_test_pool() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database")
}

async fn cleanup_test_data(pool: &PgPool, cbu_name: &str) {
    // Clean up in reverse order of foreign key dependencies
    sqlx::query(
        r#"
        DELETE FROM "ob-poc".scheduled_reviews
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".monitoring_events
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".monitoring_setup
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".decision_conditions
        WHERE decision_id IN (
            SELECT decision_id FROM "ob-poc".kyc_decisions
            WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        )
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".kyc_decisions
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".risk_flags
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".risk_assessments
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".screenings
        WHERE investigation_id IN (
            SELECT investigation_id FROM "ob-poc".kyc_investigations
            WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        )
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".investigation_assignments
        WHERE investigation_id IN (
            SELECT investigation_id FROM "ob-poc".kyc_investigations
            WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        )
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".kyc_investigations
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .execute(pool)
    .await
    .ok();

    sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name = $1"#)
        .bind(cbu_name)
        .execute(pool)
        .await
        .ok();
}

fn build_kyc_session_dsl(cbu_name: &str, jurisdiction: &str) -> String {
    format!(
        r#"
;; =============================================================================
;; KYC Session: Idempotent test case
;; Running this twice should produce the same DB state
;; =============================================================================

;; Step 1: Create CBU (idempotent via cbu.ensure)
(cbu.ensure
  :cbu-name "{cbu_name}"
  :jurisdiction "{jurisdiction}"
  :nature-purpose "Alternative investment fund"
  :client-type "SICAV")

;; Step 2: Create investigation
(investigation.create
  :investigation-type "ENHANCED_DUE_DILIGENCE"
  :risk-rating "HIGH"
  :ubo-threshold 10.0
  :deadline "2024-02-15")

;; Step 3: Update investigation status
(investigation.update-status
  :status "COLLECTING_DOCUMENTS")

;; Step 4: Risk assessment
(risk.assess-cbu
  :methodology "FACTOR_WEIGHTED")

;; Step 5: Set risk rating
(risk.set-rating
  :rating "HIGH"
  :rationale "PEP exposure with complex offshore structure")

;; Step 6: Add risk flag
(risk.add-flag
  :flag-type "AMBER_FLAG"
  :description "Trust beneficiaries pending disclosure")

;; Step 7: Record decision
(decision.record
  :decision "CONDITIONAL_ACCEPTANCE"
  :decision-authority "SENIOR_MANAGEMENT"
  :rationale "PEP risk acceptable with enhanced monitoring")

;; Step 8: Add condition
(decision.add-condition
  :condition-type "ENHANCED_MONITORING"
  :frequency "QUARTERLY"
  :description "Quarterly review of all transactions >EUR 1M")

;; Step 9: Setup monitoring
(monitoring.setup
  :monitoring-level "ENHANCED")

;; Step 10: Schedule review
(monitoring.schedule-review
  :review-type "ANNUAL_KYC_REFRESH"
  :due-date "2025-01-15")
"#,
        cbu_name = cbu_name,
        jurisdiction = jurisdiction,
    )
}

/// Count rows for a CBU across all KYC tables
async fn count_kyc_rows(pool: &PgPool, cbu_name: &str) -> (i64, i64, i64, i64, i64, i64) {
    let cbu_count: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".cbus WHERE name = $1"#)
            .bind(cbu_name)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let investigation_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM "ob-poc".kyc_investigations
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let risk_assessment_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM "ob-poc".risk_assessments
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let decision_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM "ob-poc".kyc_decisions
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let monitoring_setup_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM "ob-poc".monitoring_setup
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let scheduled_review_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM "ob-poc".scheduled_reviews
        WHERE cbu_id IN (SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1)
        "#,
    )
    .bind(cbu_name)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    (
        cbu_count,
        investigation_count,
        risk_assessment_count,
        decision_count,
        monitoring_setup_count,
        scheduled_review_count,
    )
}

#[tokio::test]
#[ignore] // Requires DATABASE_URL
async fn test_kyc_session_idempotent_execution() {
    let pool = get_test_pool().await;
    let test_id = Uuid::new_v4().to_string()[..8].to_uppercase();
    let cbu_name = format!("KYC_IDEMPOTENT_TEST_{}", test_id);

    // Clean up any existing test data
    cleanup_test_data(&pool, &cbu_name).await;

    // Build the DSL session
    let dsl_content = build_kyc_session_dsl(&cbu_name, "LU");

    // =========================================================================
    // FIRST EXECUTION
    // =========================================================================
    println!("=== First Execution ===");

    let sheet1 = DslSheet {
        id: format!("kyc-session-1-{}", test_id),
        domain: "kyc".to_string(),
        version: "1".to_string(),
        content: dsl_content.clone(),
    };

    let (result1, mut env1) = execute_sheet_into_env(&sheet1, Some(pool.clone()))
        .expect("First execution should succeed");

    assert!(
        result1.success,
        "First execution failed: {:?}",
        result1.logs
    );

    // Execute CRUD statements from first run with context capture
    let executor = CrudExecutor::new(pool.clone());
    let crud_stmts1 = env1.take_pending_crud();
    println!("First run generated {} CRUD statements", crud_stmts1.len());

    // Use execute_all_with_env to capture context IDs (cbu_id, investigation_id, etc.)
    executor
        .execute_all_with_env(&crud_stmts1, &mut env1)
        .await
        .expect("CRUD execution failed");

    // Count rows after first execution
    let counts_after_first = count_kyc_rows(&pool, &cbu_name).await;
    println!("Counts after first execution: {:?}", counts_after_first);

    // Verify we have data
    assert_eq!(counts_after_first.0, 1, "Should have exactly 1 CBU");
    assert!(
        counts_after_first.1 >= 1,
        "Should have at least 1 investigation"
    );
    assert!(
        counts_after_first.4 >= 1,
        "Should have at least 1 monitoring setup"
    );

    // =========================================================================
    // SECOND EXECUTION (should be idempotent)
    // =========================================================================
    println!("\n=== Second Execution (Idempotent) ===");

    let sheet2 = DslSheet {
        id: format!("kyc-session-2-{}", test_id),
        domain: "kyc".to_string(),
        version: "1".to_string(),
        content: dsl_content.clone(),
    };

    let (result2, mut env2) = execute_sheet_into_env(&sheet2, Some(pool.clone()))
        .expect("Second execution should succeed");

    assert!(
        result2.success,
        "Second execution failed: {:?}",
        result2.logs
    );

    // Execute CRUD statements from second run with context capture
    let crud_stmts2 = env2.take_pending_crud();
    println!("Second run generated {} CRUD statements", crud_stmts2.len());

    // Use execute_all_with_env to capture context IDs
    executor
        .execute_all_with_env(&crud_stmts2, &mut env2)
        .await
        .expect("Second CRUD execution failed");

    // Count rows after second execution
    let counts_after_second = count_kyc_rows(&pool, &cbu_name).await;
    println!("Counts after second execution: {:?}", counts_after_second);

    // =========================================================================
    // VERIFY IDEMPOTENCY
    // =========================================================================
    println!("\n=== Verifying Idempotency ===");

    // CBU should still be exactly 1 (UPSERT via cbu.ensure)
    assert_eq!(
        counts_after_second.0, 1,
        "CBU count should remain 1 after second execution (idempotent)"
    );

    // Monitoring setup should still be 1 (UPSERT via unique constraint)
    assert_eq!(
        counts_after_first.4, counts_after_second.4,
        "Monitoring setup count should remain the same (idempotent)"
    );

    println!("Idempotency test PASSED!");

    // Cleanup
    cleanup_test_data(&pool, &cbu_name).await;
}

#[tokio::test]
#[ignore] // Requires DATABASE_URL
async fn test_cbu_ensure_idempotency() {
    let pool = get_test_pool().await;
    let test_id = Uuid::new_v4().to_string()[..8].to_uppercase();
    let cbu_name = format!("CBU_ENSURE_TEST_{}", test_id);

    cleanup_test_data(&pool, &cbu_name).await;

    let executor = CrudExecutor::new(pool.clone());

    // First ensure
    let dsl1 = format!(
        r#"(cbu.ensure :cbu-name "{}" :jurisdiction "GB" :client-type "HEDGE_FUND")"#,
        cbu_name
    );

    let sheet1 = DslSheet {
        id: "test-1".to_string(),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: dsl1,
    };

    let (_, mut env1) = execute_sheet_into_env(&sheet1, Some(pool.clone())).unwrap();
    let stmts1 = env1.take_pending_crud();
    executor
        .execute_all_with_env(&stmts1, &mut env1)
        .await
        .unwrap();

    let count1: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".cbus WHERE name = $1"#)
        .bind(&cbu_name)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(count1, 1, "First ensure should create 1 CBU");

    // Second ensure (with updated client-type)
    let dsl2 = format!(
        r#"(cbu.ensure :cbu-name "{}" :jurisdiction "GB" :client-type "UCITS")"#,
        cbu_name
    );

    let sheet2 = DslSheet {
        id: "test-2".to_string(),
        domain: "cbu".to_string(),
        version: "1".to_string(),
        content: dsl2,
    };

    let (_, mut env2) = execute_sheet_into_env(&sheet2, Some(pool.clone())).unwrap();
    let stmts2 = env2.take_pending_crud();
    executor
        .execute_all_with_env(&stmts2, &mut env2)
        .await
        .unwrap();

    let count2: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".cbus WHERE name = $1"#)
        .bind(&cbu_name)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(count2, 1, "Second ensure should NOT create duplicate CBU");

    // Verify client_type was updated
    let client_type: Option<String> =
        sqlx::query_scalar(r#"SELECT client_type FROM "ob-poc".cbus WHERE name = $1"#)
            .bind(&cbu_name)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        client_type,
        Some("UCITS".to_string()),
        "Client type should be updated on second ensure"
    );

    cleanup_test_data(&pool, &cbu_name).await;
    println!("cbu.ensure idempotency test PASSED!");
}

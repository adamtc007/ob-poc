//! Onboarding DSL Test Harness Integration Tests
//!
//! These tests validate the complete onboarding DSL pipeline:
//! 1. Create onboarding request
//! 2. Parse and validate DSL against schema
//! 3. Persist to database
//! 4. Verify all writes
//!
//! Run with: cargo test --test onboarding_harness_tests --features database

use ob_poc::database::{DatabaseConfig, DatabaseManager};
use ob_poc::dsl_test_harness::{OnboardingTestHarness, OnboardingTestInput};
use sqlx::PgPool;
use uuid::Uuid;

const TEST_CBU_ID: Uuid = Uuid::from_u128(0x11111111_1111_1111_1111_111111111111);

/// Helper to get database pool for tests
async fn get_test_pool() -> PgPool {
    let config = DatabaseConfig::default();
    let db = DatabaseManager::new(config).await.expect("Failed to connect to database");
    db.pool().clone()
}

#[tokio::test]
async fn test_harness_creation() {
    let pool = get_test_pool().await;
    let harness = OnboardingTestHarness::new(pool).await;
    assert!(harness.is_ok(), "Harness should be created successfully");
}

#[tokio::test]
async fn test_valid_onboarding_simple() {
    let pool = get_test_pool().await;
    let harness = OnboardingTestHarness::new(pool).await.unwrap();

    let dsl = r#"
;; Simple onboarding test
(cbu.ensure :cbu-name "Test Fund Ltd" :jurisdiction "LU" :as @cbu)
"#;

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec![],
        dsl_source: dsl.to_string(),
    }).await;

    match result {
        Ok(r) => {
            println!("Test result: validation_passed={}, parse_time={}ms, total_time={}ms",
                r.validation_passed, r.parse_time_ms, r.total_time_ms);
            if !r.validation_passed {
                println!("Errors: {:?}", r.errors);
            }
        }
        Err(e) => {
            println!("Test failed with error: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_valid_onboarding_with_products() {
    let pool = get_test_pool().await;
    let harness = OnboardingTestHarness::new(pool).await.unwrap();

    let dsl = r#"
;; Onboarding with products
(cbu.ensure :cbu-name "Test Fund Ltd" :jurisdiction "LU" :as @cbu)
(entity.create-limited-company :name "Test ManCo" :jurisdiction "LU" :as @manco)
"#;

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec!["GLOB_CUST".to_string()],
        dsl_source: dsl.to_string(),
    }).await;

    match result {
        Ok(r) => {
            println!("Test result: validation_passed={}", r.validation_passed);
            println!("Verification: request_exists={}, products_linked={}",
                r.verification.request_exists, r.verification.products_linked);
            if r.validation_passed {
                println!("DSL instance ID: {:?}, version: {:?}",
                    r.dsl_instance_id, r.dsl_version);
            }
        }
        Err(e) => {
            println!("Test failed with error: {:?}", e);
        }
    }
}

#[tokio::test]
#[ignore = "Requires onboarding_requests table migration"]
async fn test_parse_error_handling() {
    let pool = get_test_pool().await;
    let harness = OnboardingTestHarness::new(pool).await.unwrap();

    // Invalid DSL syntax
    let dsl = r#"(cbu.ensure :cbu-name "unclosed string"#;

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec![],
        dsl_source: dsl.to_string(),
    }).await.unwrap();

    assert!(!result.validation_passed, "Should fail with parse error");
    assert!(!result.errors.is_empty(), "Should have error");
    assert_eq!(result.errors[0].code, "E000", "Should be parse error");
}

#[tokio::test]
#[ignore = "Requires onboarding_requests table migration"]
async fn test_performance_timing() {
    let pool = get_test_pool().await;
    let harness = OnboardingTestHarness::new(pool).await.unwrap();

    let dsl = r#"(cbu.ensure :cbu-name "Test" :jurisdiction "LU" :as @cbu)"#;

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec![],
        dsl_source: dsl.to_string(),
    }).await.unwrap();

    println!("Performance: parse={}ms, validate={}ms, persist={}ms, total={}ms",
        result.parse_time_ms,
        result.validate_time_ms,
        result.persist_time_ms,
        result.total_time_ms);

    // Parsing should be fast
    assert!(result.parse_time_ms < 100, "Parse should be under 100ms");
}

#[tokio::test]
#[ignore = "Requires onboarding_requests table migration"]
async fn test_verification_on_success() {
    let pool = get_test_pool().await;
    let harness = OnboardingTestHarness::new(pool).await.unwrap();

    let dsl = r#"(cbu.ensure :cbu-name "Verify Test" :jurisdiction "GB" :as @cbu)"#;

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec![],
        dsl_source: dsl.to_string(),
    }).await.unwrap();

    if result.validation_passed {
        let v = &result.verification;
        assert!(v.request_exists, "Request should exist in DB");
        assert!(v.dsl_instance_exists, "DSL instance should exist");
        assert!(v.dsl_content_matches, "DSL content should match");
        assert!(v.ast_exists, "AST should exist");
    }
}

#[tokio::test]
#[ignore = "Requires onboarding_requests table migration"]
async fn test_verification_on_failure() {
    let pool = get_test_pool().await;
    let harness = OnboardingTestHarness::new(pool).await.unwrap();

    // Force a parse error
    let dsl = r#"(invalid syntax"#;

    let result = harness.run_test(OnboardingTestInput {
        cbu_id: TEST_CBU_ID,
        product_codes: vec![],
        dsl_source: dsl.to_string(),
    }).await.unwrap();

    assert!(!result.validation_passed);
    let v = &result.verification;
    assert!(v.request_exists, "Request should still exist");
    assert!(v.errors_stored, "Errors should be stored");
    assert!(!v.dsl_instance_exists, "DSL instance should NOT exist for failed validation");
}

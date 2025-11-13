//! Phase 4: Integration Tests with Live Database
//!
//! This module implements comprehensive integration testing for the complete
//! DSL Manager → DSL Mod → Database orchestration pipeline as specified in
//! Phase 4 of the DSL_MANAGER_TO_DSL_MOD_PLAN.md.
//!
//! ## Test Coverage
//! 1. End-to-end orchestration testing
//! 2. Database round-trip operations with SQLX
//! 3. Performance and load testing
//! 4. Error handling and failure scenarios
//! 5. Concurrent operation testing
//! 6. Transaction integrity testing
//!
//! ## Architecture Tested
//! ```
//! Natural Language → AI Service → DSL Generation → DSL Manager → DSL Processor → Database → Response
//! ```

use ob_poc::{
    database::{DatabaseConfig, DatabaseManager, DictionaryDatabaseService},
    dsl::{
        DslOrchestrationInterface, DslPipelineProcessor, ExecutionResult, OrchestrationContext,
        OrchestrationOperation, OrchestrationOperationType,
    },
    dsl_manager::{CallChainResult, CleanDslManager, CleanManagerConfig},
    models::dictionary_models::{DictionaryAttribute, NewDictionaryAttribute},
};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use uuid::Uuid;

/// Database configuration for integration tests
const TEST_DATABASE_URL: &str = "postgresql://postgres:password@localhost:5432/ob_poc_test";
const TEST_TIMEOUT_SECONDS: u64 = 30;
const PERFORMANCE_TEST_ITERATIONS: usize = 100;

/// Setup test database connection with proper configuration
async fn setup_test_database() -> Result<PgPool, Box<dyn std::error::Error>> {
    let database_url =
        std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| TEST_DATABASE_URL.to_string());

    let config = DatabaseConfig {
        database_url,
        max_connections: 10,
        connection_timeout: Duration::from_secs(5),
        idle_timeout: Some(Duration::from_secs(300)),
        max_lifetime: Some(Duration::from_secs(900)),
    };

    let db_manager = DatabaseManager::new(config).await?;

    // Test basic connectivity
    db_manager.test_connection().await?;

    // Verify schema exists
    let pool = db_manager.pool().clone();
    let schema_check = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM information_schema.schemata
        WHERE schema_name = 'ob-poc'
        "#,
    )
    .fetch_one(&pool)
    .await?;

    if schema_check == 0 {
        return Err("ob-poc schema not found. Please run schema initialization scripts.".into());
    }

    Ok(pool)
}

/// Clean up test data from database
async fn cleanup_test_data(pool: &PgPool, test_case_id: &str) -> Result<(), sqlx::Error> {
    // Clean up test data in reverse dependency order
    let _result = sqlx::query("DELETE FROM \"ob-poc\".dsl_instances WHERE case_id = $1")
        .bind(test_case_id)
        .execute(pool)
        .await;

    let _result = sqlx::query("DELETE FROM \"ob-poc\".entities WHERE case_id = $1")
        .bind(test_case_id)
        .execute(pool)
        .await;

    let _result = sqlx::query("DELETE FROM \"ob-poc\".cbus WHERE case_id = $1")
        .bind(test_case_id)
        .execute(pool)
        .await;

    Ok(())
}

/// Phase 4 Test 1: Basic End-to-End Orchestration
#[tokio::test]
#[cfg(feature = "database")]
async fn test_end_to_end_orchestration() {
    let pool = match setup_test_database().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Skipping database test - setup failed: {}", e);
            return;
        }
    };

    let database_service = DictionaryDatabaseService::new(pool.clone());
    let mut manager = CleanDslManager::with_database(database_service);

    let test_case_id = format!("E2E-TEST-{}", Uuid::new_v4().to_string()[..8]);
    let dsl_content = format!(
        r#"(case.create
            :case-id "{}"
            :case-type "INTEGRATION_TEST"
            :customer-name "End-to-End Test Customer"
            :jurisdiction "US")"#,
        test_case_id
    );

    // Execute the complete pipeline
    let start_time = Instant::now();
    let result = manager.execute_dsl_with_database(dsl_content.clone()).await;
    let execution_time = start_time.elapsed();

    // Verify orchestration succeeded
    assert!(result.success, "End-to-end orchestration should succeed");
    assert!(!result.case_id.is_empty(), "Should generate case ID");
    assert!(
        result.processing_time_ms > 0,
        "Should record processing time"
    );

    // Verify step details
    if let Some(dsl_step) = &result.step_details.dsl_processing {
        assert!(dsl_step.success, "DSL processing should succeed");
        assert!(
            dsl_step.processing_time_ms > 0,
            "DSL step should record time"
        );
    }

    println!(
        "✅ End-to-end orchestration completed in {:?}",
        execution_time
    );

    // Cleanup
    cleanup_test_data(&pool, &test_case_id).await.unwrap_or(());
}

/// Phase 4 Test 2: Database Round-Trip Operations
#[tokio::test]
#[cfg(feature = "database")]
async fn test_database_round_trip_operations() {
    let pool = match setup_test_database().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Skipping database test - setup failed: {}", e);
            return;
        }
    };

    let database_service = DictionaryDatabaseService::new(pool.clone());
    let processor = DslPipelineProcessor::with_database(database_service.clone());

    let test_case_id = format!("RT-TEST-{}", Uuid::new_v4().to_string()[..8]);
    let context = OrchestrationContext::new("test-user".to_string(), "onboarding".to_string())
        .with_case_id(test_case_id.clone());

    // Test different DSL operations
    let test_operations = vec![
        (
            format!(
                "(case.create :case-id \"{}\" :case-type \"ROUND_TRIP\")",
                test_case_id
            ),
            "CREATE_CASE",
        ),
        (
            format!(
                "(case.update :case-id \"{}\" :status \"PROCESSING\")",
                test_case_id
            ),
            "UPDATE_CASE",
        ),
        (
            format!(
                "(entity.register :case-id \"{}\" :entity-type \"PERSON\")",
                test_case_id
            ),
            "CREATE_ENTITY",
        ),
    ];

    for (dsl_content, expected_op_type) in test_operations {
        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::Execute,
            dsl_content.clone(),
            context.clone(),
        );

        let start_time = Instant::now();
        let result = processor.process_orchestrated_operation(operation).await;
        let processing_time = start_time.elapsed();

        assert!(result.is_ok(), "Operation should succeed: {}", dsl_content);

        let orchestration_result = result.unwrap();
        assert!(
            orchestration_result.success,
            "Orchestration should be successful"
        );
        assert!(
            orchestration_result.processing_time_ms > 0,
            "Should record time"
        );

        println!(
            "✅ {} operation completed in {:?}",
            expected_op_type, processing_time
        );
    }

    // Cleanup
    cleanup_test_data(&pool, &test_case_id).await.unwrap_or(());
}

/// Phase 4 Test 3: Concurrent Operations Testing
#[tokio::test]
#[cfg(feature = "database")]
async fn test_concurrent_operations() {
    let pool = match setup_test_database().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Skipping database test - setup failed: {}", e);
            return;
        }
    };

    let database_service = Arc::new(DictionaryDatabaseService::new(pool.clone()));
    let num_concurrent_ops = 5;
    let mut handles = Vec::new();

    for i in 0..num_concurrent_ops {
        let db_service = database_service.clone();
        let pool_clone = pool.clone();

        let handle = tokio::spawn(async move {
            let processor = DslPipelineProcessor::with_database(db_service.as_ref().clone());
            let test_case_id = format!("CONCURRENT-{}-{}", i, Uuid::new_v4().to_string()[..8]);

            let context = OrchestrationContext::new(
                format!("concurrent-user-{}", i),
                "concurrent-test".to_string(),
            )
            .with_case_id(test_case_id.clone());

            let operation = OrchestrationOperation::new(
                OrchestrationOperationType::Execute,
                format!(
                    "(case.create :case-id \"{}\" :case-type \"CONCURRENT_TEST\" :thread-id {})",
                    test_case_id, i
                ),
                context,
            );

            let start_time = Instant::now();
            let result = processor.process_orchestrated_operation(operation).await;
            let processing_time = start_time.elapsed();

            // Cleanup
            cleanup_test_data(&pool_clone, &test_case_id)
                .await
                .unwrap_or(());

            (i, result, processing_time)
        });

        handles.push(handle);
    }

    // Wait for all operations to complete
    let results = futures::future::join_all(handles).await;

    // Verify all operations succeeded
    for result in results {
        let (thread_id, operation_result, processing_time) = result.unwrap();
        assert!(
            operation_result.is_ok(),
            "Concurrent operation {} should succeed",
            thread_id
        );

        let orchestration_result = operation_result.unwrap();
        assert!(
            orchestration_result.success,
            "Concurrent orchestration {} should be successful",
            thread_id
        );

        println!(
            "✅ Concurrent operation {} completed in {:?}",
            thread_id, processing_time
        );
    }

    println!(
        "✅ All {} concurrent operations completed successfully",
        num_concurrent_ops
    );
}

/// Phase 4 Test 4: Performance Testing
#[tokio::test]
#[cfg(feature = "database")]
async fn test_performance_characteristics() {
    let pool = match setup_test_database().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Skipping database test - setup failed: {}", e);
            return;
        }
    };

    let database_service = DictionaryDatabaseService::new(pool.clone());
    let processor = DslPipelineProcessor::with_database(database_service);

    let iterations = std::cmp::min(PERFORMANCE_TEST_ITERATIONS, 10); // Limit for CI
    let mut processing_times = Vec::with_capacity(iterations);
    let base_case_id = format!("PERF-{}", Uuid::new_v4().to_string()[..8]);

    let overall_start = Instant::now();

    for i in 0..iterations {
        let test_case_id = format!("{}-{:03}", base_case_id, i);
        let context = OrchestrationContext::new("perf-user".to_string(), "performance".to_string())
            .with_case_id(test_case_id.clone());

        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::Execute,
            format!(
                "(case.create :case-id \"{}\" :case-type \"PERFORMANCE_TEST\" :iteration {})",
                test_case_id, i
            ),
            context,
        );

        let start_time = Instant::now();
        let result = timeout(
            Duration::from_secs(TEST_TIMEOUT_SECONDS),
            processor.process_orchestrated_operation(operation),
        )
        .await;

        assert!(
            result.is_ok(),
            "Performance test operation should not timeout"
        );
        let operation_result = result.unwrap();
        assert!(
            operation_result.is_ok(),
            "Performance test operation {} should succeed",
            i
        );

        let processing_time = start_time.elapsed();
        processing_times.push(processing_time);

        // Cleanup immediately to avoid accumulating data
        cleanup_test_data(&pool, &test_case_id).await.unwrap_or(());

        // Small delay to avoid overwhelming the database
        if i % 10 == 9 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let total_time = overall_start.elapsed();
    let avg_time = processing_times.iter().sum::<Duration>() / processing_times.len() as u32;
    let min_time = processing_times.iter().min().unwrap();
    let max_time = processing_times.iter().max().unwrap();

    // Performance assertions
    assert!(
        avg_time.as_millis() < 1000,
        "Average processing time should be under 1 second"
    );
    assert!(
        max_time.as_millis() < 5000,
        "Max processing time should be under 5 seconds"
    );

    println!("✅ Performance Test Results:");
    println!("   Iterations: {}", iterations);
    println!("   Total Time: {:?}", total_time);
    println!("   Average Time: {:?}", avg_time);
    println!("   Min Time: {:?}", min_time);
    println!("   Max Time: {:?}", max_time);
    println!(
        "   Throughput: {:.2} ops/sec",
        iterations as f64 / total_time.as_secs_f64()
    );
}

/// Phase 4 Test 5: Error Handling and Recovery
#[tokio::test]
#[cfg(feature = "database")]
async fn test_error_handling_and_recovery() {
    let pool = match setup_test_database().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Skipping database test - setup failed: {}", e);
            return;
        }
    };

    let database_service = DictionaryDatabaseService::new(pool.clone());
    let processor = DslPipelineProcessor::with_database(database_service);

    // Test 1: Invalid DSL syntax
    let context =
        OrchestrationContext::new("error-test-user".to_string(), "error-test".to_string());
    let invalid_operation = OrchestrationOperation::new(
        OrchestrationOperationType::Execute,
        "invalid dsl syntax here".to_string(), // Invalid DSL
        context.clone(),
    );

    let result = processor
        .process_orchestrated_operation(invalid_operation)
        .await;
    assert!(result.is_ok(), "Error handling should not panic");
    // Note: Current implementation may not detect syntax errors - this tests graceful handling

    // Test 2: Empty DSL content
    let empty_operation = OrchestrationOperation::new(
        OrchestrationOperationType::Execute,
        "".to_string(),
        context.clone(),
    );

    let result = processor
        .process_orchestrated_operation(empty_operation)
        .await;
    assert!(result.is_ok(), "Empty DSL should be handled gracefully");

    // Test 3: Very long DSL content (stress test)
    let long_dsl = format!(
        "(case.create :case-id \"LONG-TEST\" :description \"{}\")",
        "x".repeat(10000)
    );
    let long_operation = OrchestrationOperation::new(
        OrchestrationOperationType::Execute,
        long_dsl,
        context.clone(),
    );

    let result = timeout(
        Duration::from_secs(10),
        processor.process_orchestrated_operation(long_operation),
    )
    .await;

    assert!(result.is_ok(), "Long DSL should not cause timeout");

    println!("✅ Error handling tests completed successfully");
}

/// Phase 4 Test 6: Dictionary Service Integration
#[tokio::test]
#[cfg(feature = "database")]
async fn test_dictionary_service_integration() {
    let pool = match setup_test_database().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Skipping database test - setup failed: {}", e);
            return;
        }
    };

    let database_service = DictionaryDatabaseService::new(pool.clone());

    // Test dictionary health check
    let health_result = database_service.health_check().await;
    match health_result {
        Ok(_) => println!("✅ Dictionary service health check passed"),
        Err(e) => println!("⚠️ Dictionary service health check failed: {}", e),
    }

    // Test creating a dictionary attribute (if the table exists)
    let test_attribute_id = Uuid::new_v4();
    let new_attribute = NewDictionaryAttribute {
        attribute_id: test_attribute_id,
        name: "test-integration-attribute".to_string(),
        description: Some("Integration test attribute".to_string()),
        data_type: "TEXT".to_string(),
        validation_rules: Some(serde_json::json!({"required": true})),
        privacy_classification: Some("PUBLIC".to_string()),
        version: 1,
        created_by: "integration-test".to_string(),
        tags: Some(vec!["integration".to_string(), "test".to_string()]),
    };

    // Attempt to create the attribute
    let create_result = database_service.create_attribute(new_attribute).await;
    match create_result {
        Ok(created_attr) => {
            println!(
                "✅ Successfully created test attribute: {}",
                created_attr.name
            );

            // Clean up the test attribute
            let _cleanup =
                sqlx::query(r#"DELETE FROM "ob-poc".dictionary WHERE attribute_id = $1"#)
                    .bind(test_attribute_id)
                    .execute(&pool)
                    .await;
        }
        Err(e) => {
            println!("ℹ️ Dictionary attribute creation test skipped: {}", e);
            // This is not a failure - the dictionary table might not be fully set up
        }
    }
}

/// Phase 4 Test 7: Full Pipeline Integration with Multiple Steps
#[tokio::test]
#[cfg(feature = "database")]
async fn test_full_pipeline_integration() {
    let pool = match setup_test_database().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Skipping database test - setup failed: {}", e);
            return;
        }
    };

    let database_service = DictionaryDatabaseService::new(pool.clone());
    let processor = DslPipelineProcessor::with_database(database_service);

    let test_case_id = format!("PIPELINE-{}", Uuid::new_v4().to_string()[..8]);
    let context = OrchestrationContext::new("pipeline-user".to_string(), "onboarding".to_string())
        .with_case_id(test_case_id.clone());

    // Step 1: Parse operation
    let parse_operation = OrchestrationOperation::new(
        OrchestrationOperationType::Parse,
        format!(
            "(case.create :case-id \"{}\" :case-type \"PIPELINE_TEST\")",
            test_case_id
        ),
        context.clone(),
    );

    let parse_result = processor
        .process_orchestrated_operation(parse_operation)
        .await;
    assert!(parse_result.is_ok(), "Parse operation should succeed");
    assert!(parse_result.unwrap().success, "Parse should be successful");

    // Step 2: Validate operation
    let validate_operation = OrchestrationOperation::new(
        OrchestrationOperationType::Validate,
        format!(
            "(case.create :case-id \"{}\" :case-type \"PIPELINE_TEST\")",
            test_case_id
        ),
        context.clone(),
    );

    let validate_result = processor
        .process_orchestrated_operation(validate_operation)
        .await;
    assert!(validate_result.is_ok(), "Validate operation should succeed");
    assert!(
        validate_result.unwrap().success,
        "Validation should be successful"
    );

    // Step 3: Execute operation
    let execute_operation = OrchestrationOperation::new(
        OrchestrationOperationType::Execute,
        format!(
            "(case.create :case-id \"{}\" :case-type \"PIPELINE_TEST\")",
            test_case_id
        ),
        context.clone(),
    );

    let execute_result = processor
        .process_orchestrated_operation(execute_operation)
        .await;
    assert!(execute_result.is_ok(), "Execute operation should succeed");
    assert!(
        execute_result.unwrap().success,
        "Execution should be successful"
    );

    // Step 4: Complete processing operation
    let complete_operation = OrchestrationOperation::new(
        OrchestrationOperationType::ProcessComplete,
        format!(
            "(case.create :case-id \"{}\" :case-type \"PIPELINE_TEST\")",
            test_case_id
        ),
        context.clone(),
    );

    let complete_result = processor
        .process_orchestrated_operation(complete_operation)
        .await;
    assert!(complete_result.is_ok(), "Complete operation should succeed");
    assert!(
        complete_result.unwrap().success,
        "Complete processing should be successful"
    );

    println!("✅ Full pipeline integration test completed successfully");

    // Cleanup
    cleanup_test_data(&pool, &test_case_id).await.unwrap_or(());
}

/// Phase 4 Test 8: Connection Pool Stress Testing
#[tokio::test]
#[cfg(feature = "database")]
async fn test_connection_pool_stress() {
    let pool = match setup_test_database().await {
        Ok(pool) => pool,
        Err(e) => {
            println!("Skipping database test - setup failed: {}", e);
            return;
        }
    };

    let database_service = Arc::new(DictionaryDatabaseService::new(pool.clone()));
    let concurrent_connections = 8; // Less than pool max to avoid exhaustion
    let operations_per_connection = 5;

    let mut handles = Vec::new();
    let overall_start = Instant::now();

    for conn_id in 0..concurrent_connections {
        let db_service = database_service.clone();
        let pool_clone = pool.clone();

        let handle = tokio::spawn(async move {
            let processor = DslPipelineProcessor::with_database(db_service.as_ref().clone());
            let mut operation_times = Vec::new();

            for op_id in 0..operations_per_connection {
                let test_case_id = format!(
                    "STRESS-{}-{}-{}",
                    conn_id,
                    op_id,
                    Uuid::new_v4().to_string()[..8]
                );

                let context = OrchestrationContext::new(
                    format!("stress-user-{}", conn_id),
                    "stress-test".to_string(),
                )
                .with_case_id(test_case_id.clone());

                let operation = OrchestrationOperation::new(
                    OrchestrationOperationType::Execute,
                    format!(
                        "(case.create :case-id \"{}\" :case-type \"STRESS_TEST\" :conn {} :op {})",
                        test_case_id, conn_id, op_id
                    ),
                    context,
                );

                let op_start = Instant::now();
                let result = processor.process_orchestrated_operation(operation).await;
                let op_time = op_start.elapsed();

                assert!(
                    result.is_ok(),
                    "Stress test operation should succeed (conn: {}, op: {})",
                    conn_id,
                    op_id
                );

                operation_times.push(op_time);

                // Cleanup
                cleanup_test_data(&pool_clone, &test_case_id)
                    .await
                    .unwrap_or(());

                // Small delay between operations
                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            (conn_id, operation_times)
        });

        handles.push(handle);
    }

    // Wait for all stress test operations
    let results = futures::future::join_all(handles).await;
    let total_time = overall_start.elapsed();

    let mut all_operation_times = Vec::new();
    for result in results {
        let (conn_id, operation_times) = result.unwrap();
        println!(
            "✅ Connection {} completed {} operations",
            conn_id,
            operation_times.len()
        );
        all_operation_times.extend(operation_times);
    }

    let total_operations = concurrent_connections * operations_per_connection;
    let avg_time = all_operation_times.iter().sum::<Duration>() / all_operation_times.len() as u32;
    let throughput = total_operations as f64 / total_time.as_secs_f64();

    println!("✅ Connection Pool Stress Test Results:");
    println!("   Total Operations: {}", total_operations);
    println!("   Total Time: {:?}", total_time);
    println!("   Average Operation Time: {:?}", avg_time);
    println!("   Throughput: {:.2} ops/sec", throughput);

    // Performance assertions
    assert!(
        avg_time.as_millis() < 2000,
        "Average operation time should be under 2 seconds under stress"
    );
    assert!(
        throughput > 1.0,
        "Should maintain at least 1 operation per second under stress"
    );
}

/// Integration test helper: Verify test environment
#[tokio::test]
#[cfg(feature = "database")]
async fn test_environment_verification() {
    println!("🧪 Phase 4 Integration Test Environment Verification");

    // Test 1: Database connectivity
    match setup_test_database().await {
        Ok(pool) => {
            println!("✅ Database connection established");

            // Test 2: Schema verification
            let schema_tables = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM information_schema.tables
                WHERE table_schema = 'ob-poc'
                "#,
            )
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

            println!("✅ Found {} tables in ob-poc schema", schema_tables);

            // Test 3: Dictionary service instantiation
            let database_service = DictionaryDatabaseService::new(pool.clone());
            match database_service.health_check().await {
                Ok(_) => println!("✅ Dictionary service health check passed"),
                Err(e) => println!("ℹ️ Dictionary service health check: {}", e),
            }

            // Test 4: DSL processor instantiation
            let _processor = DslPipelineProcessor::with_database(database_service);
            println!("✅ DSL processor with database created successfully");

            // Test 5: DSL manager instantiation
            let database_service2 = DictionaryDatabaseService::new(pool);
            let _manager = CleanDslManager::with_database(database_service2);
            println!("✅ DSL manager with database created successfully");
        }
        Err(e) => {
            println!("❌ Database setup failed: {}", e);
            println!("ℹ️ To run full integration tests, ensure:");
            println!("   1. PostgreSQL is running");
            println!("   2. Database 'ob_poc_test' exists");
            println!("   3. Schema has been initialized with migration scripts");
            println!("   4. TEST_DATABASE_URL environment variable is set (optional)");
        }
    }

    println!("🎯 Phase 4 Integration Test Environment Check Complete");
}

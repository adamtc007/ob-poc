//! Phase 3 Unit Tests - Database Integration Without Live Database
//!
//! These tests verify the database integration architecture and patterns
//! without requiring a live database connection. They test the wiring,
//! interfaces, and mock behavior of the DSL Manager → DSL Mod → Database orchestration.

use ob_poc::{
    database::DictionaryDatabaseService,
    dsl::{
        DslOrchestrationInterface, DslPipelineProcessor, OrchestrationContext,
        OrchestrationOperation, OrchestrationOperationType,
    },
    dsl_manager::{CleanDslManager, CleanManagerConfig},
};

/// Test 1: DSL Pipeline Processor Database Integration Patterns
#[tokio::test]
async fn test_dsl_processor_database_patterns() {
    // Test processor without database
    let processor = DslPipelineProcessor::new();
    assert!(!processor.has_database());
    assert!(processor.database_service().is_none());

    // Test processor with mock database service
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let processor_with_db = DslPipelineProcessor::with_database(mock_db_service);

    assert!(processor_with_db.has_database());
    assert!(processor_with_db.database_service().is_some());
}

/// Test 2: Clean DSL Manager Database Integration Patterns
#[tokio::test]
async fn test_clean_manager_database_patterns() {
    // Test manager without database
    let manager = CleanDslManager::new();
    assert!(!manager.has_database());
    assert!(manager.database_service().is_none());

    // Test manager with mock database service
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let manager_with_db = CleanDslManager::with_database(mock_db_service);

    assert!(manager_with_db.has_database());
    assert!(manager_with_db.database_service().is_some());
}

/// Test 3: Configuration and Database Integration
#[tokio::test]
async fn test_config_and_database_integration() {
    let config = CleanManagerConfig {
        enable_detailed_logging: true,
        enable_metrics: true,
        max_processing_time_seconds: 30,
        enable_auto_cleanup: true,
    };

    let mock_db_service = DictionaryDatabaseService::new_mock();
    let manager = CleanDslManager::with_config_and_database(config, mock_db_service);

    assert!(manager.has_database());
    // Test that configuration is properly integrated
    // (The actual config is private, but we can test behavior)

    let health_result = manager.health_check().await;
    assert!(health_result.system_operational);
}

/// Test 4: Orchestration Interface Database Operations (Mock)
#[tokio::test]
async fn test_orchestration_database_operations_mock() {
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let processor = DslPipelineProcessor::with_database(mock_db_service);

    let context = OrchestrationContext::new("test-user".to_string(), "test-domain".to_string())
        .with_case_id("TEST-CASE-001".to_string());

    let operation = OrchestrationOperation::new(
        OrchestrationOperationType::Execute,
        "(case.create :case-id \"TEST-CASE-001\" :case-type \"ONBOARDING\")".to_string(),
        context,
    );

    // This should work with mock database (won't actually connect)
    let result = processor.process_orchestrated_operation(operation).await;

    assert!(result.is_ok());
    let orchestration_result = result.unwrap();
    assert!(orchestration_result.success);
    assert!(!orchestration_result.operation_id.is_empty());
    assert!(orchestration_result.processing_time_ms > 0);
}

/// Test 5: DSL Execution Results with Mock Database
#[tokio::test]
async fn test_dsl_execution_with_mock_database() {
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let processor = DslPipelineProcessor::with_database(mock_db_service);

    let context = OrchestrationContext::new("test-user".to_string(), "onboarding".to_string())
        .with_case_id("EXEC-TEST-001".to_string());

    let execution_result = processor
        .execute_orchestrated_dsl(
            "(case.create :case-id \"EXEC-TEST-001\" :customer-name \"Test Customer\")",
            context,
        )
        .await;

    assert!(execution_result.is_ok());
    let result = execution_result.unwrap();

    assert!(result.success);
    assert!(result.output.is_some());
    assert!(result.execution_time_ms > 0);
    assert!(!result.database_operations.is_empty());

    // Verify database operation structure
    let db_op = &result.database_operations[0];
    assert_eq!(db_op.operation_type, "CREATE_CASE");
    assert!(db_op.success);
    assert_eq!(db_op.affected_count, 1);
}

/// Test 6: Different DSL Operation Types Database Mapping
#[tokio::test]
async fn test_dsl_operation_types_database_mapping() {
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let processor = DslPipelineProcessor::with_database(mock_db_service);

    let base_context = OrchestrationContext::new("test".to_string(), "test".to_string());

    // Test different DSL operations map to correct database operations
    let test_cases = vec![
        ("(case.create :case-id \"TEST-001\")", "CREATE_CASE"),
        ("(case.update :case-id \"TEST-002\")", "UPDATE_CASE"),
        ("(entity.register :entity-id \"ENT-001\")", "CREATE_ENTITY"),
        ("(kyc.start :case-id \"KYC-001\")", "START_KYC"),
        ("(unknown.operation :id \"UNK-001\")", "UNKNOWN_OPERATION"),
    ];

    for (dsl_content, expected_op_type) in test_cases {
        let context = base_context.clone().with_case_id("TEST-CASE".to_string());

        let result = processor
            .execute_orchestrated_dsl(dsl_content, context)
            .await
            .unwrap();

        assert!(result.success);
        assert!(!result.database_operations.is_empty());
        assert_eq!(
            result.database_operations[0].operation_type,
            expected_op_type
        );
    }
}

/// Test 7: Error Handling Without Database
#[tokio::test]
async fn test_error_handling_without_database() {
    let processor = DslPipelineProcessor::new(); // No database

    let context = OrchestrationContext::new("test".to_string(), "test".to_string())
        .with_case_id("NO-DB-TEST".to_string());

    let result = processor
        .execute_orchestrated_dsl("(case.create :case-id \"NO-DB-TEST\")", context)
        .await;

    assert!(result.is_ok());
    let execution_result = result.unwrap();

    // Should still succeed but with mock operations
    assert!(execution_result.success);
    assert!(!execution_result.database_operations.is_empty());

    // Should have mock database operation
    let db_op = &execution_result.database_operations[0];
    assert_eq!(db_op.operation_type, "MOCK_EXECUTE");
    assert!(db_op.success);
}

/// Test 8: DSL Manager Database Execution Flow
#[tokio::test]
async fn test_dsl_manager_database_execution_flow() {
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let mut manager = CleanDslManager::with_database(mock_db_service);

    // Test successful execution with database
    let dsl_content =
        "(case.create :case-id \"FLOW-TEST-001\" :case-type \"ONBOARDING\")".to_string();
    let result = manager.execute_dsl_with_database(dsl_content).await;

    assert!(result.success);
    assert!(!result.case_id.is_empty());
    assert!(result.processing_time_ms > 0);
    assert!(result.step_details.dsl_processing.is_some());

    // Test error when no database
    let mut manager_no_db = CleanDslManager::new();
    let dsl_content = "(case.create :case-id \"NO-DB-TEST\")".to_string();
    let result = manager_no_db.execute_dsl_with_database(dsl_content).await;

    assert!(!result.success);
    assert!(!result.errors.is_empty());
    assert!(result.errors[0].contains("No database connectivity"));
}

/// Test 9: Orchestration Operation Types
#[tokio::test]
async fn test_orchestration_operation_types() {
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let processor = DslPipelineProcessor::with_database(mock_db_service);

    let context = OrchestrationContext::new("test".to_string(), "test".to_string());
    let dsl_content = "(case.create :case-id \"OP-TYPE-TEST\")".to_string();

    // Test Parse operation
    let parse_op = OrchestrationOperation::new(
        OrchestrationOperationType::Parse,
        dsl_content.clone(),
        context.clone(),
    );
    let parse_result = processor
        .process_orchestrated_operation(parse_op)
        .await
        .unwrap();
    assert!(parse_result.success);

    // Test Validate operation
    let validate_op = OrchestrationOperation::new(
        OrchestrationOperationType::Validate,
        dsl_content.clone(),
        context.clone(),
    );
    let validate_result = processor
        .process_orchestrated_operation(validate_op)
        .await
        .unwrap();
    assert!(validate_result.success);

    // Test Execute operation
    let execute_op = OrchestrationOperation::new(
        OrchestrationOperationType::Execute,
        dsl_content.clone(),
        context.clone(),
    );
    let execute_result = processor
        .process_orchestrated_operation(execute_op)
        .await
        .unwrap();
    assert!(execute_result.success);

    // Test ProcessComplete operation (full pipeline)
    let complete_op = OrchestrationOperation::new(
        OrchestrationOperationType::ProcessComplete,
        dsl_content,
        context,
    );
    let complete_result = processor
        .process_orchestrated_operation(complete_op)
        .await
        .unwrap();
    assert!(complete_result.success);
}

/// Test 10: Pipeline Configuration with Database
#[tokio::test]
async fn test_pipeline_configuration_with_database() {
    use ob_poc::dsl::PipelineConfig;

    let config = PipelineConfig {
        enable_strict_validation: true,
        fail_fast: false,
        enable_detailed_logging: true,
        max_step_time_seconds: 30,
        enable_metrics: true,
    };

    let mock_db_service = DictionaryDatabaseService::new_mock();
    let processor = DslPipelineProcessor::with_config_and_database(config, mock_db_service);

    assert!(processor.has_database());

    // Test that configured processor works correctly
    let context = OrchestrationContext::new("config-test".to_string(), "test".to_string());
    let operation = OrchestrationOperation::new(
        OrchestrationOperationType::Execute,
        "(case.create :case-id \"CONFIG-TEST\")".to_string(),
        context,
    );

    let result = processor
        .process_orchestrated_operation(operation)
        .await
        .unwrap();
    assert!(result.success);

    // With metrics enabled, should have processing time
    assert!(result.processing_time_ms > 0);
}

/// Test 11: Health Check Integration
#[tokio::test]
async fn test_health_check_integration() {
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let processor = DslPipelineProcessor::with_database(mock_db_service);

    let health_result = processor.orchestration_health_check().await;
    assert!(health_result.is_ok());

    let health_status = health_result.unwrap();
    assert!(health_status.healthy);
    assert!(health_status.checked_at > 0);
}

/// Test 12: Complex DSL with Database Integration
#[tokio::test]
async fn test_complex_dsl_database_integration() {
    let mock_db_service = DictionaryDatabaseService::new_mock();
    let mut manager = CleanDslManager::with_database(mock_db_service);

    // Complex DSL with multiple operations
    let complex_dsl = r#"
    (case.create
        :case-id "COMPLEX-TEST-001"
        :case-type "ONBOARDING"
        :customer-name "Complex Test Customer"
        :jurisdiction "US"
        :risk-level "MEDIUM"
        :products ["CUSTODY", "TRADING"]
        :compliance-requirements ["KYC", "AML", "SANCTIONS"])
    "#
    .to_string();

    let result = manager.execute_dsl_with_database(complex_dsl).await;

    assert!(result.success, "Complex DSL should process successfully");
    assert!(!result.case_id.is_empty(), "Should extract case ID");
    assert!(
        result.processing_time_ms > 0,
        "Should record processing time"
    );

    // Should have processed through all steps
    if let Some(dsl_step) = &result.step_details.dsl_processing {
        assert!(dsl_step.success, "DSL processing step should succeed");
    }
}

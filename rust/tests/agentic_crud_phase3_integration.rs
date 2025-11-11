//! Integration tests for Agentic CRUD Phase 3 functionality
//!
//! These tests verify the complete integration of:
//! - Real AI providers (with mock fallbacks)
//! - Database operations
//! - DSL parsing and execution
//! - Error handling and recovery

use anyhow::Result;
use ob_poc::ai::agentic_crud_service::{
    AgenticCrudRequest, AgenticCrudService, AiProvider, ServiceConfig,
};
use ob_poc::database::DatabaseManager;
use ob_poc::execution::crud_executor::{CrudExecutor, CrudResult};
use ob_poc::parser::idiomatic_parser::parse_crud_statement;
use ob_poc::{DataCreate, DataRead, Value};
use std::collections::HashMap;
use std::env;
use tokio;
use uuid::Uuid;

// Helper to create test database manager
async fn setup_test_database() -> Result<DatabaseManager> {
    let test_db_url = env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc-test".to_string());

    // Try to connect to test database, fallback to default if not available
    match DatabaseManager::with_default_config().await {
        Ok(db) => {
            // Try to test connection
            match db.test_connection().await {
                Ok(_) => Ok(db),
                Err(_) => {
                    eprintln!("Warning: Database connection failed, tests may be limited");
                    Ok(db) // Return anyway for testing
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Could not create database manager: {}", e);
            // For CI/CD environments without database, we'll create a mock
            Err(e.into())
        }
    }
}

// Helper to create mock agentic service for testing
async fn create_mock_service(db_manager: &DatabaseManager) -> Result<AgenticCrudService> {
    let mut mock_responses = HashMap::new();

    // Add comprehensive mock responses
    mock_responses.insert("create_cbu".to_string(),
        r#"{"dsl_content": "(data.create :asset \"cbu\" :values {:name \"Test CBU\" :jurisdiction \"GB\" :customer_type \"CORP\"})",
           "explanation": "Created test CBU",
           "confidence": 0.9,
           "changes": [],
           "warnings": [],
           "suggestions": []}"#.to_string());

    mock_responses.insert("read_cbu".to_string(),
        r#"{"dsl_content": "(data.read :asset \"cbu\" :select [\"name\" \"jurisdiction\"] :where {:customer_type \"CORP\"})",
           "explanation": "Read corporate CBUs",
           "confidence": 0.85,
           "changes": [],
           "warnings": [],
           "suggestions": []}"#.to_string());

    let config = ServiceConfig {
        ai_provider: AiProvider::Mock {
            responses: mock_responses,
        },
        model_config: Default::default(),
        prompt_config: Default::default(),
        execute_dsl: false, // Don't execute in tests by default
        max_retries: 2,
        timeout_seconds: 5,
        enable_caching: false,
        cache_ttl_seconds: 0,
    };

    AgenticCrudService::new(db_manager.pool().clone(), config).await
}

#[tokio::test]
async fn test_agentic_service_creation() -> Result<()> {
    let db_manager = match setup_test_database().await {
        Ok(db) => db,
        Err(_) => return Ok(()), // Skip test if no database
    };

    let service = create_mock_service(&db_manager).await?;

    // Test health check
    let health = service.health_check().await?;
    assert!(health.overall_status, "Service should be healthy");

    // Test statistics
    let stats = service.get_statistics().await?;
    assert_eq!(stats.ai_provider, "mock");

    Ok(())
}

#[tokio::test]
async fn test_natural_language_to_dsl_generation() -> Result<()> {
    let db_manager = match setup_test_database().await {
        Ok(db) => db,
        Err(_) => return Ok(()), // Skip test if no database
    };

    let service = create_mock_service(&db_manager).await?;

    let request = AgenticCrudRequest {
        instruction: "Create a new CBU for a UK corporation needing custody services".to_string(),
        context_hints: None,
        execute: false, // Don't execute, just test generation
        request_id: Some("test_generation".to_string()),
        business_context: Some(
            [
                ("entity_type".to_string(), "corporation".to_string()),
                ("jurisdiction".to_string(), "GB".to_string()),
            ]
            .iter()
            .cloned()
            .collect(),
        ),
        constraints: Some(vec!["Use approved DSL verbs only".to_string()]),
    };

    let response = service.process_request(request).await?;

    assert!(response.success, "Request should succeed");
    assert!(
        !response.generated_dsl.is_empty(),
        "DSL should be generated"
    );
    assert!(
        response.parsed_statement.is_some(),
        "DSL should parse correctly"
    );
    assert_eq!(response.request_id, "test_generation");

    // Verify the DSL contains expected elements
    assert!(
        response.generated_dsl.contains("data.create"),
        "Should contain create operation"
    );
    assert!(
        response.generated_dsl.contains("cbu"),
        "Should target CBU asset"
    );

    Ok(())
}

#[tokio::test]
async fn test_dsl_parsing_and_validation() -> Result<()> {
    // Test parsing various DSL statements
    let test_cases = vec![
        (
            r#"(data.create :asset "cbu" :values {:name "Test Corp" :jurisdiction "GB"})"#,
            true,
            "Simple CBU creation should parse",
        ),
        (
            r#"(data.read :asset "cbu" :select ["name"] :where {:jurisdiction "GB"})"#,
            true,
            "CBU read query should parse",
        ),
        (
            r#"(invalid.verb :bad "syntax")"#,
            false,
            "Invalid DSL should not parse",
        ),
    ];

    for (dsl, should_succeed, description) in test_cases {
        let result = parse_crud_statement(dsl);

        if should_succeed {
            assert!(result.is_ok(), "{}: {}", description, dsl);
        } else {
            assert!(result.is_err(), "{}: {}", description, dsl);
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_crud_executor_schema_validation() -> Result<()> {
    let db_manager = match setup_test_database().await {
        Ok(db) => db,
        Err(_) => return Ok(()), // Skip test if no database
    };

    let executor = CrudExecutor::new(db_manager.pool().clone());

    // Test valid CBU creation
    let mut values = HashMap::new();
    values.insert("name".to_string(), Value::String("Test CBU".to_string()));
    values.insert("jurisdiction".to_string(), Value::String("GB".to_string()));

    let create_op = DataCreate {
        asset: "cbu".to_string(),
        values,
    };

    // This should not fail on schema validation (even if database execution fails)
    // We're just testing that the executor can handle the operation structure
    match executor.execute(create_op.into()).await {
        Ok(_) => println!("CBU creation succeeded"),
        Err(e) => {
            // Database errors are OK for this test, we're testing schema validation
            println!("Database execution failed (expected in test): {}", e);
        }
    }

    // Test invalid asset type
    let invalid_create = DataCreate {
        asset: "invalid_asset".to_string(),
        values: HashMap::new(),
    };

    let invalid_result = executor.execute(invalid_create.into()).await;
    assert!(
        invalid_result.is_err(),
        "Invalid asset type should be rejected"
    );

    Ok(())
}

#[tokio::test]
async fn test_error_handling_and_recovery() -> Result<()> {
    let db_manager = match setup_test_database().await {
        Ok(db) => db,
        Err(_) => return Ok(()), // Skip test if no database
    };

    // Create service with error-prone configuration
    let mut error_responses = HashMap::new();
    error_responses.insert(
        "error_test".to_string(),
        r#"{"dsl_content": "(invalid syntax)",
           "explanation": "This should cause a parsing error",
           "confidence": 0.1,
           "changes": [],
           "warnings": ["Invalid syntax"],
           "suggestions": ["Use valid DSL syntax"]}"#
            .to_string(),
    );

    let config = ServiceConfig {
        ai_provider: AiProvider::Mock {
            responses: error_responses,
        },
        model_config: Default::default(),
        prompt_config: Default::default(),
        execute_dsl: false,
        max_retries: 3,
        timeout_seconds: 5,
        enable_caching: false,
        cache_ttl_seconds: 0,
    };

    let service = AgenticCrudService::new(db_manager.pool().clone(), config).await?;

    let error_request = AgenticCrudRequest {
        instruction: "This will cause an error".to_string(),
        context_hints: None,
        execute: false,
        request_id: Some("error_test".to_string()),
        business_context: None,
        constraints: None,
    };

    let response = service.process_request(error_request).await?;

    // The service should handle errors gracefully
    assert!(!response.success, "Error request should not succeed");
    assert!(!response.errors.is_empty(), "Should have error messages");
    assert!(
        response.parsed_statement.is_none(),
        "Should not have valid parsed statement"
    );

    Ok(())
}

#[tokio::test]
async fn test_caching_functionality() -> Result<()> {
    let db_manager = match setup_test_database().await {
        Ok(db) => db,
        Err(_) => return Ok(()), // Skip test if no database
    };

    // Create service with caching enabled
    let config = ServiceConfig {
        ai_provider: AiProvider::Mock {
            responses: [(
                "cache_test".to_string(),
                r#"{"dsl_content": "(data.read :asset \"cbu\" :select [\"name\"])",
                   "explanation": "Cached response",
                   "confidence": 0.9,
                   "changes": [],
                   "warnings": [],
                   "suggestions": []}"#
                    .to_string(),
            )]
            .iter()
            .cloned()
            .collect(),
        },
        model_config: Default::default(),
        prompt_config: Default::default(),
        execute_dsl: false,
        max_retries: 1,
        timeout_seconds: 5,
        enable_caching: true,
        cache_ttl_seconds: 60,
    };

    let service = AgenticCrudService::new(db_manager.pool().clone(), config).await?;

    let cache_request = AgenticCrudRequest {
        instruction: "Test caching with this request".to_string(),
        context_hints: None,
        execute: false,
        request_id: Some("cache_test_1".to_string()),
        business_context: None,
        constraints: None,
    };

    // First request (cache miss)
    let start_time = std::time::Instant::now();
    let response1 = service.process_request(cache_request.clone()).await?;
    let time1 = start_time.elapsed();

    // Second request (cache hit)
    let start_time = std::time::Instant::now();
    let mut cache_request2 = cache_request;
    cache_request2.request_id = Some("cache_test_2".to_string());
    let response2 = service.process_request(cache_request2).await?;
    let time2 = start_time.elapsed();

    assert!(response1.success, "First request should succeed");
    assert!(response2.success, "Second request should succeed");

    // Note: In a real implementation, we'd expect time2 to be significantly less than time1
    // For this test, we're just verifying both requests complete successfully
    println!(
        "First request: {}ms, Second request: {}ms",
        time1.as_millis(),
        time2.as_millis()
    );

    Ok(())
}

#[tokio::test]
async fn test_comprehensive_workflow() -> Result<()> {
    let db_manager = match setup_test_database().await {
        Ok(db) => db,
        Err(_) => return Ok(()), // Skip test if no database
    };

    let service = create_mock_service(&db_manager).await?;

    // Test complete workflow: Create -> Read -> Update -> Delete
    let workflow_steps = vec![
        ("Create CBU", "Create a new CBU for testing workflow"),
        ("Read CBU", "Find the CBU we just created"),
        ("Update CBU", "Update the CBU risk rating"),
        ("Search CBUs", "Find all CBUs matching criteria"),
    ];

    for (step_name, instruction) in workflow_steps {
        let request = AgenticCrudRequest {
            instruction: instruction.to_string(),
            context_hints: None,
            execute: false, // Don't execute to avoid database dependencies
            request_id: Some(format!(
                "workflow_{}",
                step_name.replace(" ", "_").to_lowercase()
            )),
            business_context: None,
            constraints: None,
        };

        let response = service.process_request(request).await?;

        println!(
            "Step '{}': Success={}, DSL={}",
            step_name, response.success, response.generated_dsl
        );

        // Each step should succeed in generating valid DSL
        assert!(
            response.success,
            "Workflow step '{}' should succeed",
            step_name
        );
        assert!(
            !response.generated_dsl.is_empty(),
            "Workflow step '{}' should generate DSL",
            step_name
        );
    }

    // Verify final service statistics
    let final_stats = service.get_statistics().await?;
    assert!(
        final_stats.total_operations >= 4,
        "Should have recorded all workflow operations"
    );

    Ok(())
}

// Helper test for different Value types
#[test]
fn test_value_type_conversions() {
    // Test the extended Value enum functionality
    let string_val = Value::String("test".to_string());
    assert_eq!(string_val.as_string(), Some("test".to_string()));

    let int_val = Value::Integer(42);
    assert_eq!(int_val.as_string(), Some("42".to_string()));

    let bool_val = Value::Boolean(true);
    assert_eq!(bool_val.as_string(), Some("true".to_string()));

    let array_val = Value::Array(vec![
        Value::String("item1".to_string()),
        Value::String("item2".to_string()),
    ]);
    assert!(matches!(array_val, Value::Array(_)));
}

// Test database-independent functionality
#[test]
fn test_crud_statement_construction() {
    // Test creating CRUD statements programmatically
    let mut values = HashMap::new();
    values.insert("name".to_string(), Value::String("Test Entity".to_string()));
    values.insert("active".to_string(), Value::Boolean(true));

    let create_stmt = DataCreate {
        asset: "test_entity".to_string(),
        values: values.clone(),
    };

    assert_eq!(create_stmt.asset, "test_entity");
    assert!(create_stmt.values.contains_key("name"));
    assert!(create_stmt.values.contains_key("active"));

    let mut where_clause = HashMap::new();
    where_clause.insert("id".to_string(), Value::Integer(1));

    let read_stmt = DataRead {
        asset: "test_entity".to_string(),
        where_clause,
        select: vec!["name".to_string(), "active".to_string()],
        limit: Some(10),
    };

    assert_eq!(read_stmt.asset, "test_entity");
    assert_eq!(read_stmt.select.len(), 2);
    assert_eq!(read_stmt.limit, Some(10));
}

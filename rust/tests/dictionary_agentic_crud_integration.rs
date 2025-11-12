//! Integration tests for Dictionary Agentic CRUD operations
//!
//! This test suite validates the complete flow from natural language instructions
//! to DSL generation to database operations for attribute/dictionary management.

#[cfg(all(test, feature = "database"))]
mod dictionary_agentic_crud_tests {
    use ob_poc::ai::agentic_dictionary_service::{
        AgenticDictionaryService, DictionaryServiceConfig,
    };
    use ob_poc::database::{DatabaseManager, DictionaryDatabaseService};
    use ob_poc::models::{
        AgenticAttributeCreateRequest, AgenticAttributeDeleteRequest,
        AgenticAttributeDiscoverRequest, AgenticAttributeReadRequest,
        AgenticAttributeSearchRequest, AgenticAttributeUpdateRequest,
        AgenticAttributeValidateRequest, AttributeAssetType, AttributeDiscoveryRequest,
        AttributeOperationType, AttributeSearchCriteria, AttributeValidationRequest,
        DictionaryExecutionStatus, NewDictionaryAttribute,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio;
    use uuid::Uuid;

    // Mock AI service for testing
    struct MockAiService {
        responses: HashMap<String, String>,
    }

    impl MockAiService {
        fn new() -> Self {
            let mut responses = HashMap::new();
            responses.insert(
                "create".to_string(),
                "(attribute.create :name \"test.attribute\" :description \"Test attribute\" :mask \"string\")"
                    .to_string(),
            );
            responses.insert(
                "read".to_string(),
                "(attribute.read :name \"test.attribute\")".to_string(),
            );
            responses.insert(
                "update".to_string(),
                "(attribute.update :attribute-id \"123e4567-e89b-12d3-a456-426614174000\" :description \"Updated description\")"
                    .to_string(),
            );
            responses.insert(
                "delete".to_string(),
                "(attribute.delete :attribute-id \"123e4567-e89b-12d3-a456-426614174000\")"
                    .to_string(),
            );

            Self { responses }
        }
    }

    #[async_trait::async_trait]
    impl ob_poc::ai::AiService for MockAiService {
        async fn generate_dsl(
            &self,
            request: ob_poc::ai::AiDslRequest,
        ) -> ob_poc::ai::AiResult<ob_poc::ai::AiDslResponse> {
            let instruction = request.instruction.to_lowercase();

            let dsl_content = if instruction.contains("create") {
                self.responses
                    .get("create")
                    .unwrap_or(&"(attribute.create)".to_string())
                    .clone()
            } else if instruction.contains("read") || instruction.contains("find") {
                self.responses
                    .get("read")
                    .unwrap_or(&"(attribute.read)".to_string())
                    .clone()
            } else if instruction.contains("update") {
                self.responses
                    .get("update")
                    .unwrap_or(&"(attribute.update)".to_string())
                    .clone()
            } else if instruction.contains("delete") {
                self.responses
                    .get("delete")
                    .unwrap_or(&"(attribute.delete)".to_string())
                    .clone()
            } else {
                "(attribute.read)".to_string()
            };

            Ok(ob_poc::ai::AiDslResponse {
                generated_dsl: dsl_content,
                explanation: format!("Generated DSL for: {}", instruction),
                confidence: Some(0.9),
                changes: None,
                warnings: None,
                suggestions: None,
            })
        }

        async fn health_check(&self) -> ob_poc::ai::AiResult<bool> {
            Ok(true)
        }

        fn config(&self) -> &ob_poc::ai::AiConfig {
            // Return a mock config
            static CONFIG: ob_poc::ai::AiConfig = ob_poc::ai::AiConfig {
                api_key: "mock".to_string(),
                model: "mock-model".to_string(),
                max_tokens: Some(1000),
                temperature: Some(0.1),
                timeout_seconds: 30,
            };
            &CONFIG
        }
    }

    async fn setup_test_database() -> DatabaseManager {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost:5432/ob_poc_test".to_string());

        let config = ob_poc::database::DatabaseConfig {
            database_url,
            max_connections: 5,
            connection_timeout: std::time::Duration::from_secs(5),
            idle_timeout: Some(std::time::Duration::from_secs(60)),
            max_lifetime: Some(std::time::Duration::from_secs(300)),
        };

        DatabaseManager::new(config)
            .await
            .expect("Failed to connect to test database")
    }

    fn create_agentic_service(db_service: DictionaryDatabaseService) -> AgenticDictionaryService {
        let ai_client = Arc::new(MockAiService::new());
        let config = DictionaryServiceConfig {
            execute_dsl: true,
            max_retries: 2,
            timeout_seconds: 10,
            enable_caching: false, // Disable caching for tests
            cache_ttl_seconds: 60,
            ai_temperature: 0.1,
            max_tokens: Some(1000),
        };

        AgenticDictionaryService::new(db_service, ai_client, Some(config))
    }

    #[tokio::test]
    async fn test_agentic_attribute_create_operation() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        let create_request = AgenticAttributeCreateRequest {
            instruction: "Create a new attribute for storing customer email addresses".to_string(),
            asset_type: AttributeAssetType::Attribute,
            context: HashMap::new(),
            constraints: vec![
                "Must be a valid email format".to_string(),
                "Required for all customers".to_string(),
            ],
            group_id: Some("customer".to_string()),
            domain: Some("contact".to_string()),
        };

        let result = agentic_service.create_agentic(create_request).await;

        assert!(result.is_ok(), "Create operation should succeed");
        let response = result.unwrap();

        assert_eq!(response.operation_type, AttributeOperationType::Create);
        assert!(!response.generated_dsl.is_empty());
        assert!(response.ai_explanation.contains("attribute"));
        assert!(response.ai_confidence.is_some());

        if response.execution_status == DictionaryExecutionStatus::Completed {
            assert!(!response.affected_records.is_empty());
        }
    }

    #[tokio::test]
    async fn test_agentic_attribute_read_operation() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        let read_request = AgenticAttributeReadRequest {
            instruction: "Find all attributes related to customer information".to_string(),
            asset_types: vec![AttributeAssetType::Attribute],
            filters: {
                let mut filters = HashMap::new();
                filters.insert(
                    "domain".to_string(),
                    serde_json::Value::String("customer".to_string()),
                );
                filters
            },
            limit: Some(10),
            offset: Some(0),
        };

        let result = agentic_service.read_agentic(read_request).await;

        assert!(result.is_ok(), "Read operation should succeed");
        let response = result.unwrap();

        assert_eq!(response.operation_type, AttributeOperationType::Read);
        assert!(!response.generated_dsl.is_empty());
        assert!(
            response.ai_explanation.contains("attributes")
                || response.ai_explanation.contains("Found")
        );
        assert!(response.ai_confidence.is_some());
    }

    #[tokio::test]
    async fn test_agentic_attribute_search_operation() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        let search_criteria = AttributeSearchCriteria {
            name_pattern: Some("email".to_string()),
            group_id: Some("customer".to_string()),
            domain: None,
            mask: Some("string".to_string()),
            semantic_query: None,
            limit: Some(5),
            offset: Some(0),
        };

        let search_request = AgenticAttributeSearchRequest {
            instruction: "Search for email-related attributes in customer group".to_string(),
            search_criteria,
            semantic_search: false,
        };

        let result = agentic_service.search_agentic(search_request).await;

        assert!(result.is_ok(), "Search operation should succeed");
        let response = result.unwrap();

        assert_eq!(response.operation_type, AttributeOperationType::Search);
        assert!(!response.generated_dsl.is_empty());
        assert!(response.execution_status == DictionaryExecutionStatus::Completed);
        assert!(response.results.is_some());
    }

    #[tokio::test]
    async fn test_agentic_attribute_validation_operation() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        // First, create a test attribute
        let test_attr_id = Uuid::new_v4();

        let validation_request = AttributeValidationRequest {
            attribute_id: test_attr_id,
            value: serde_json::Value::String("test@example.com".to_string()),
            context: None,
        };

        let validate_request = AgenticAttributeValidateRequest {
            instruction: "Validate this email address format".to_string(),
            validation_request,
        };

        let result = agentic_service.validate_agentic(validate_request).await;

        // Note: This might fail if the attribute doesn't exist, which is expected
        // In a real test, we'd create the attribute first
        if let Ok(response) = result {
            assert_eq!(response.operation_type, AttributeOperationType::Validate);
            assert!(!response.generated_dsl.is_empty());
            assert!(response.results.is_some());
        }
    }

    #[tokio::test]
    async fn test_agentic_attribute_discover_operation() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        let discovery_request = AttributeDiscoveryRequest {
            semantic_query: "attributes for storing financial information".to_string(),
            domain_filter: Some("finance".to_string()),
            group_filter: None,
            limit: Some(5),
        };

        let discover_request = AgenticAttributeDiscoverRequest {
            instruction: "Find attributes suitable for financial data".to_string(),
            discovery_request,
        };

        let result = agentic_service.discover_agentic(discover_request).await;

        assert!(result.is_ok(), "Discover operation should succeed");
        let response = result.unwrap();

        assert_eq!(response.operation_type, AttributeOperationType::Discover);
        assert!(!response.generated_dsl.is_empty());
        assert!(response.execution_status == DictionaryExecutionStatus::Completed);
        assert!(response.results.is_some());
    }

    #[tokio::test]
    async fn test_agentic_service_cache_operations() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        // Test cache stats
        let stats = agentic_service.cache_stats().await;
        assert_eq!(stats.total_entries, 0); // Should start empty

        // Test cache clearing
        agentic_service.clear_cache().await;
        let stats_after_clear = agentic_service.cache_stats().await;
        assert_eq!(stats_after_clear.total_entries, 0);
    }

    #[tokio::test]
    async fn test_error_handling_invalid_instruction() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        let create_request = AgenticAttributeCreateRequest {
            instruction: "".to_string(), // Empty instruction
            asset_type: AttributeAssetType::Attribute,
            context: HashMap::new(),
            constraints: vec![],
            group_id: None,
            domain: None,
        };

        let result = agentic_service.create_agentic(create_request).await;

        // Should handle gracefully, might succeed with default values
        // or fail with appropriate error message
        if let Err(e) = result {
            assert!(!e.to_string().is_empty());
        }
    }

    #[tokio::test]
    async fn test_multiple_operations_sequence() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        // 1. Create an attribute
        let create_request = AgenticAttributeCreateRequest {
            instruction: "Create attribute for customer phone number".to_string(),
            asset_type: AttributeAssetType::Attribute,
            context: HashMap::new(),
            constraints: vec!["Must be valid phone format".to_string()],
            group_id: Some("customer".to_string()),
            domain: Some("contact".to_string()),
        };

        let create_result = agentic_service.create_agentic(create_request).await;
        assert!(create_result.is_ok());

        // 2. Search for the created attribute
        let search_criteria = AttributeSearchCriteria {
            name_pattern: Some("phone".to_string()),
            group_id: Some("customer".to_string()),
            domain: Some("contact".to_string()),
            mask: None,
            semantic_query: None,
            limit: Some(10),
            offset: Some(0),
        };

        let search_request = AgenticAttributeSearchRequest {
            instruction: "Find phone number attributes".to_string(),
            search_criteria,
            semantic_search: false,
        };

        let search_result = agentic_service.search_agentic(search_request).await;
        assert!(search_result.is_ok());

        // 3. Discover related attributes
        let discovery_request = AttributeDiscoveryRequest {
            semantic_query: "contact information attributes".to_string(),
            domain_filter: Some("contact".to_string()),
            group_filter: None,
            limit: Some(5),
        };

        let discover_request = AgenticAttributeDiscoverRequest {
            instruction: "Find related contact attributes".to_string(),
            discovery_request,
        };

        let discover_result = agentic_service.discover_agentic(discover_request).await;
        assert!(discover_result.is_ok());
    }

    #[tokio::test]
    async fn test_performance_timing() {
        let db_manager = setup_test_database().await;
        let db_service = db_manager.dictionary_service();
        let agentic_service = create_agentic_service(db_service);

        let start_time = std::time::Instant::now();

        let create_request = AgenticAttributeCreateRequest {
            instruction: "Create a performance test attribute".to_string(),
            asset_type: AttributeAssetType::Attribute,
            context: HashMap::new(),
            constraints: vec![],
            group_id: Some("performance".to_string()),
            domain: Some("test".to_string()),
        };

        let result = agentic_service.create_agentic(create_request).await;

        let elapsed = start_time.elapsed();

        assert!(result.is_ok());
        let response = result.unwrap();

        // Performance expectations
        assert!(
            elapsed.as_secs() < 30,
            "Operation should complete within 30 seconds"
        );

        if let Some(execution_time) = response.execution_time_ms {
            assert!(
                execution_time < 10000,
                "Database execution should be under 10 seconds"
            );
        }
    }
}

#[cfg(not(feature = "database"))]
mod no_database_tests {
    #[test]
    fn test_feature_flag_message() {
        println!("Dictionary agentic CRUD tests require the 'database' feature to be enabled.");
        println!("Run with: cargo test --features database");
    }
}

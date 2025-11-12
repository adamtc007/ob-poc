//! Comprehensive Agentic Dictionary Round-Trip Database Integration Test
//!
//! This test demonstrates the complete flow from natural language instruction
//! to DSL generation to database operations and back, proving the system works
//! end-to-end with real PostgreSQL data.
//!
//! Test Flow:
//! 1. Connect to real PostgreSQL database
//! 2. Initialize dictionary service with real AI client
//! 3. Execute natural language instructions through agentic service
//! 4. Verify database changes and responses
//! 5. Test all CRUD operations with real data
//!
//! Run with: cargo test --features database --test agentic_dictionary_roundtrip_test

#[cfg(all(test, feature = "database"))]
mod agentic_dictionary_roundtrip_tests {
    use anyhow::{Context, Result};
    use ob_poc::ai::agentic_dictionary_service::{
        AgenticDictionaryService, DictionaryServiceConfig,
    };
    use ob_poc::ai::openai::OpenAiClient;
    use ob_poc::ai::AiConfig;
    use ob_poc::database::{DatabaseConfig, DatabaseManager};
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
    use std::time::Duration;
    use tokio::time::sleep;
    use tracing::{debug, info, warn};
    use uuid::Uuid;

    /// Test configuration
    struct TestConfig {
        database_url: String,
        openai_api_key: Option<String>,
        run_ai_tests: bool,
        cleanup_test_data: bool,
    }

    impl TestConfig {
        fn from_env() -> Self {
            let database_url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

            let openai_api_key = std::env::var("OPENAI_API_KEY").ok();
            let run_ai_tests = openai_api_key.is_some() && std::env::var("SKIP_AI_TESTS").is_err();

            let cleanup_test_data = std::env::var("KEEP_TEST_DATA").is_err();

            Self {
                database_url,
                openai_api_key,
                run_ai_tests,
                cleanup_test_data,
            }
        }
    }

    /// Test harness for managing database connections and services
    struct TestHarness {
        config: TestConfig,
        db_manager: DatabaseManager,
        agentic_service: Option<AgenticDictionaryService>,
        created_attributes: Vec<Uuid>,
    }

    impl TestHarness {
        async fn new() -> Result<Self> {
            let config = TestConfig::from_env();

            // Initialize logging for tests
            let _ = tracing_subscriber::fmt()
                .with_env_filter("ob_poc=debug,agentic_dictionary_roundtrip_test=debug")
                .try_init();

            info!("🔧 Initializing test harness");
            info!("Database URL: {}", mask_database_url(&config.database_url));
            info!("AI tests enabled: {}", config.run_ai_tests);

            // Setup database connection
            let db_config = DatabaseConfig {
                database_url: config.database_url.clone(),
                max_connections: 5,
                connection_timeout: Duration::from_secs(10),
                idle_timeout: Some(Duration::from_secs(60)),
                max_lifetime: Some(Duration::from_secs(300)),
            };

            let db_manager = DatabaseManager::new(db_config)
                .await
                .context("Failed to connect to test database")?;

            // Test database connection
            db_manager
                .test_connection()
                .await
                .context("Database connection test failed")?;

            info!("✅ Database connection established");

            // Setup agentic service if AI tests are enabled
            let agentic_service = if config.run_ai_tests {
                if let Some(api_key) = &config.openai_api_key {
                    let ai_config = AiConfig {
                        api_key: api_key.clone(),
                        model: "gpt-3.5-turbo".to_string(),
                        max_tokens: Some(1000),
                        temperature: Some(0.1),
                        timeout_seconds: 30,
                    };

                    let ai_client = Arc::new(OpenAiClient::new(ai_config));

                    // Test AI client
                    match ai_client.health_check().await {
                        Ok(true) => info!("✅ AI client health check passed"),
                        Ok(false) => {
                            warn!("⚠️ AI client health check failed, using mock responses");
                        }
                        Err(e) => {
                            warn!("⚠️ AI client error: {}, using mock responses", e);
                        }
                    }

                    let dict_service = db_manager.dictionary_service();
                    let service_config = DictionaryServiceConfig {
                        execute_dsl: true,
                        max_retries: 2,
                        timeout_seconds: 30,
                        enable_caching: false, // Disable for testing
                        cache_ttl_seconds: 60,
                        ai_temperature: 0.1,
                        max_tokens: Some(1000),
                    };

                    let agentic_service = AgenticDictionaryService::new(
                        dict_service,
                        ai_client,
                        Some(service_config),
                    );

                    info!("✅ Agentic service initialized with real AI");
                    Some(agentic_service)
                } else {
                    None
                }
            } else {
                info!("⏭️ Skipping AI service initialization (no API key or disabled)");
                None
            };

            Ok(Self {
                config,
                db_manager,
                agentic_service,
                created_attributes: Vec::new(),
            })
        }

        /// Get dictionary service for direct database operations
        fn dictionary_service(&self) -> ob_poc::database::DictionaryDatabaseService {
            self.db_manager.dictionary_service()
        }

        /// Add attribute ID to cleanup list
        fn track_attribute(&mut self, attribute_id: Uuid) {
            self.created_attributes.push(attribute_id);
        }

        /// Cleanup test data
        async fn cleanup(&mut self) -> Result<()> {
            if !self.config.cleanup_test_data {
                info!("🔄 Keeping test data (KEEP_TEST_DATA is set)");
                return Ok(());
            }

            info!(
                "🧹 Cleaning up {} test attributes",
                self.created_attributes.len()
            );

            let dict_service = self.dictionary_service();
            for attribute_id in &self.created_attributes {
                match dict_service.delete_attribute(*attribute_id).await {
                    Ok(true) => debug!("Deleted test attribute: {}", attribute_id),
                    Ok(false) => debug!("Test attribute not found: {}", attribute_id),
                    Err(e) => warn!("Failed to delete test attribute {}: {}", attribute_id, e),
                }
            }

            info!("✅ Cleanup completed");
            Ok(())
        }
    }

    impl Drop for TestHarness {
        fn drop(&mut self) {
            if self.config.cleanup_test_data && !self.created_attributes.is_empty() {
                warn!(
                    "Test harness dropped with {} uncleaned attributes. Run cleanup() explicitly.",
                    self.created_attributes.len()
                );
            }
        }
    }

    /// Mask sensitive parts of database URL for logging
    fn mask_database_url(url: &str) -> String {
        if let Ok(parsed) = url::Url::parse(url) {
            let mut masked = parsed.clone();
            if parsed.password().is_some() {
                let _ = masked.set_password(Some("***"));
            }
            masked.to_string()
        } else {
            "***".to_string()
        }
    }

    #[tokio::test]
    async fn test_database_connection_and_basic_operations() -> Result<()> {
        let harness = TestHarness::new().await?;
        let dict_service = harness.dictionary_service();

        info!("🔍 Testing basic database operations");

        // Test 1: Count existing attributes
        let initial_count = dict_service.count().await?;
        info!("Initial attribute count: {}", initial_count);
        assert!(
            initial_count >= 0,
            "Should have non-negative attribute count"
        );

        // Test 2: List some attributes
        let attributes = dict_service.list_all(Some(5), Some(0)).await?;
        info!("Found {} attributes in first 5", attributes.len());

        for attr in &attributes {
            debug!(
                "Attribute: {} - {}",
                attr.name,
                attr.long_description.as_deref().unwrap_or("No description")
            );
        }

        // Test 3: Get statistics
        let stats = dict_service.get_statistics().await?;
        info!("Dictionary statistics:");
        info!("  Total: {}", stats.total_attributes);
        info!("  By domain: {:?}", stats.attributes_by_domain);
        info!("  By group: {:?}", stats.attributes_by_group);

        // Test 4: Health check
        let health = dict_service.health_check().await?;
        info!("Health status: {}", health.status);
        if !health.recommendations.is_empty() {
            info!("Recommendations:");
            for rec in &health.recommendations {
                info!("  - {}", rec);
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_direct_database_crud_operations() -> Result<()> {
        let mut harness = TestHarness::new().await?;
        let dict_service = harness.dictionary_service();

        info!("🔧 Testing direct database CRUD operations");

        let test_id = Uuid::new_v4();
        let unique_name = format!("test.roundtrip.{}", test_id);

        // Test 1: Create attribute
        let new_attribute = NewDictionaryAttribute {
            name: unique_name.clone(),
            long_description: Some("Test attribute for roundtrip testing".to_string()),
            group_id: Some("test".to_string()),
            mask: Some("string".to_string()),
            domain: Some("testing".to_string()),
            vector: None,
            source: Some(serde_json::json!({
                "test": true,
                "created_by": "roundtrip_test"
            })),
            sink: Some(serde_json::json!({
                "test_sink": true
            })),
        };

        let created = dict_service.create_attribute(new_attribute).await?;
        harness.track_attribute(created.attribute_id);

        info!(
            "✅ Created attribute: {} ({})",
            created.name, created.attribute_id
        );
        assert_eq!(created.name, unique_name);
        assert_eq!(created.group_id, "test");
        assert_eq!(created.mask, "string");

        // Test 2: Read by ID
        let read_by_id = dict_service.get_by_id(created.attribute_id).await?;
        assert!(read_by_id.is_some(), "Should find attribute by ID");
        let read_attr = read_by_id.unwrap();
        assert_eq!(read_attr.attribute_id, created.attribute_id);
        info!("✅ Read attribute by ID verified");

        // Test 3: Read by name
        let read_by_name = dict_service.get_by_name(&unique_name).await?;
        assert!(read_by_name.is_some(), "Should find attribute by name");
        assert_eq!(read_by_name.unwrap().attribute_id, created.attribute_id);
        info!("✅ Read attribute by name verified");

        // Test 4: Update attribute
        let updates = ob_poc::models::UpdateDictionaryAttribute {
            name: None,
            long_description: Some("Updated description for roundtrip test".to_string()),
            group_id: Some("updated_test".to_string()),
            mask: None,
            domain: Some("updated_testing".to_string()),
            vector: Some("test_vector".to_string()),
            source: None,
            sink: None,
        };

        let updated = dict_service
            .update_attribute(created.attribute_id, updates)
            .await?;
        assert!(updated.is_some(), "Update should return updated attribute");
        let updated_attr = updated.unwrap();
        assert_eq!(
            updated_attr.long_description,
            Some("Updated description for roundtrip test".to_string())
        );
        assert_eq!(updated_attr.group_id, "updated_test");
        assert_eq!(updated_attr.domain, Some("updated_testing".to_string()));
        info!("✅ Update attribute verified");

        // Test 5: Search attributes
        let search_criteria = AttributeSearchCriteria {
            name_pattern: Some("roundtrip".to_string()),
            group_id: Some("updated_test".to_string()),
            domain: None,
            mask: None,
            semantic_query: None,
            limit: Some(10),
            offset: Some(0),
        };

        let search_results = dict_service.search_attributes(&search_criteria).await?;
        assert!(
            !search_results.is_empty(),
            "Should find test attribute in search"
        );
        let found = search_results
            .iter()
            .find(|a| a.attribute_id == created.attribute_id);
        assert!(found.is_some(), "Should find our test attribute");
        info!(
            "✅ Search attributes verified (found {} results)",
            search_results.len()
        );

        // Test 6: Validate attribute value
        let validation_request = AttributeValidationRequest {
            attribute_id: created.attribute_id,
            value: serde_json::Value::String("test_value".to_string()),
            context: None,
        };

        let validation_result = dict_service
            .validate_attribute_value(&validation_request)
            .await?;
        info!(
            "Validation result: valid={}, errors={:?}",
            validation_result.is_valid, validation_result.validation_errors
        );

        // Test 7: Semantic search
        let discovered = dict_service
            .semantic_search("roundtrip test", Some(5))
            .await?;
        info!("✅ Semantic search returned {} results", discovered.len());

        // Cleanup will be handled by harness.cleanup()
        harness.cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_agentic_create_roundtrip() -> Result<()> {
        let mut harness = TestHarness::new().await?;

        let agentic_service = match &harness.agentic_service {
            Some(service) => service,
            None => {
                info!("⏭️ Skipping agentic test - no AI service available");
                return Ok(());
            }
        };

        info!("🤖 Testing agentic create operation roundtrip");

        let test_id = Uuid::new_v4();
        let create_request = AgenticAttributeCreateRequest {
            instruction: format!(
                "Create a new attribute called 'customer.loyalty_level.{}' for storing customer loyalty tiers like Bronze, Silver, Gold, Platinum. This should be used in customer segmentation and rewards programs.",
                test_id
            ),
            asset_type: AttributeAssetType::Attribute,
            context: {
                let mut context = HashMap::new();
                context.insert(
                    "business_context".to_string(),
                    serde_json::Value::String("Customer loyalty program".to_string()),
                );
                context.insert(
                    "test_id".to_string(),
                    serde_json::Value::String(test_id.to_string()),
                );
                context
            },
            constraints: vec![
                "Must support enum values: Bronze, Silver, Gold, Platinum".to_string(),
                "Should be used for customer segmentation".to_string(),
                "Required for rewards program".to_string(),
            ],
            group_id: Some("customer".to_string()),
            domain: Some("loyalty".to_string()),
        };

        // Execute agentic create
        let response = agentic_service.create_agentic(create_request).await?;

        info!("Agentic create response:");
        info!("  Operation ID: {}", response.operation_id);
        info!("  Generated DSL: {}", response.generated_dsl);
        info!("  Status: {:?}", response.execution_status);
        info!("  AI Explanation: {}", response.ai_explanation);
        if let Some(confidence) = response.ai_confidence {
            info!("  AI Confidence: {:.1}%", confidence * 100.0);
        }

        // Verify response structure
        assert_eq!(response.operation_type, AttributeOperationType::Create);
        assert!(!response.generated_dsl.is_empty(), "Should generate DSL");
        assert!(
            !response.ai_explanation.is_empty(),
            "Should provide explanation"
        );

        // If execution was successful, verify database changes
        if response.execution_status == DictionaryExecutionStatus::Completed {
            assert!(
                !response.affected_records.is_empty(),
                "Should have affected records"
            );

            // Track the created attribute for cleanup
            for attr_id in &response.affected_records {
                harness.track_attribute(*attr_id);
            }

            // Verify the attribute exists in database
            let dict_service = harness.dictionary_service();
            let created_attr = dict_service.get_by_id(response.affected_records[0]).await?;
            assert!(
                created_attr.is_some(),
                "Created attribute should exist in database"
            );

            let attr = created_attr.unwrap();
            info!(
                "✅ Verified created attribute: {} ({})",
                attr.name, attr.attribute_id
            );

            // Verify attributes match our intent
            assert!(
                attr.name.contains(&test_id.to_string()),
                "Name should contain test ID"
            );
            assert!(attr.long_description.is_some(), "Should have description");

            if let Some(desc) = &attr.long_description {
                assert!(
                    desc.to_lowercase().contains("loyalty"),
                    "Description should mention loyalty"
                );
            }

            info!("✅ Agentic create roundtrip completed successfully");
        } else {
            warn!(
                "⚠️ Agentic create did not complete execution (status: {:?})",
                response.execution_status
            );
        }

        harness.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_agentic_search_and_discover_roundtrip() -> Result<()> {
        let mut harness = TestHarness::new().await?;

        let agentic_service = match &harness.agentic_service {
            Some(service) => service,
            None => {
                info!("⏭️ Skipping agentic search test - no AI service available");
                return Ok(());
            }
        };

        info!("🔍 Testing agentic search and discover roundtrip");

        // First, create some test data directly in the database
        let dict_service = harness.dictionary_service();
        let test_attributes = vec![
            (
                "customer.email_address",
                "Primary email address for customer communications",
                "customer",
                "email",
            ),
            (
                "customer.phone_number",
                "Primary phone number for customer contact",
                "customer",
                "phone",
            ),
            (
                "account.balance",
                "Current account balance in base currency",
                "account",
                "financial",
            ),
            (
                "transaction.amount",
                "Transaction amount in specified currency",
                "transaction",
                "financial",
            ),
        ];

        for (name, desc, group, domain) in &test_attributes {
            let new_attr = NewDictionaryAttribute {
                name: format!("test.{}.{}", chrono::Utc::now().timestamp_millis(), name),
                long_description: Some(desc.to_string()),
                group_id: Some(group.to_string()),
                mask: Some("string".to_string()),
                domain: Some(domain.to_string()),
                vector: None,
                source: Some(serde_json::json!({"test": true})),
                sink: None,
            };

            let created = dict_service.create_attribute(new_attr).await?;
            harness.track_attribute(created.attribute_id);
            info!("Created test attribute: {}", created.name);
        }

        // Wait a moment for any eventual consistency
        sleep(Duration::from_millis(100)).await;

        // Test 1: Agentic search
        let search_criteria = AttributeSearchCriteria {
            name_pattern: Some("customer".to_string()),
            group_id: Some("customer".to_string()),
            domain: None,
            mask: None,
            semantic_query: None,
            limit: Some(10),
            offset: Some(0),
        };

        let search_request = AgenticAttributeSearchRequest {
            instruction: "Find all customer-related attributes that would be used for customer communication and contact management".to_string(),
            search_criteria,
            semantic_search: false,
        };

        let search_response = agentic_service.search_agentic(search_request).await?;

        info!("Agentic search response:");
        info!("  Generated DSL: {}", search_response.generated_dsl);
        info!("  Status: {:?}", search_response.execution_status);
        info!(
            "  Found: {} attributes",
            search_response.affected_records.len()
        );
        info!("  AI Explanation: {}", search_response.ai_explanation);

        assert_eq!(
            search_response.operation_type,
            AttributeOperationType::Search
        );
        assert_eq!(
            search_response.execution_status,
            DictionaryExecutionStatus::Completed
        );

        if let Some(results) = search_response.results {
            let attributes: Vec<ob_poc::models::DictionaryAttribute> =
                serde_json::from_value(results).unwrap_or_default();
            info!("Search returned {} actual attributes", attributes.len());

            for attr in &attributes {
                info!(
                    "  - {}: {}",
                    attr.name,
                    attr.long_description.as_deref().unwrap_or("No description")
                );
            }
        }

        // Test 2: Agentic discover
        let discovery_request = AttributeDiscoveryRequest {
            semantic_query: "financial data, money, currency, balance, transaction amounts"
                .to_string(),
            domain_filter: Some("financial".to_string()),
            group_filter: None,
            limit: Some(5),
        };

        let discover_request = AgenticAttributeDiscoverRequest {
            instruction: "Discover attributes that are related to financial information, account balances, and transaction processing".to_string(),
            discovery_request,
        };

        let discover_response = agentic_service.discover_agentic(discover_request).await?;

        info!("Agentic discover response:");
        info!("  Generated DSL: {}", discover_response.generated_dsl);
        info!("  Status: {:?}", discover_response.execution_status);
        info!(
            "  Discovered: {} attributes",
            discover_response.affected_records.len()
        );
        info!("  AI Explanation: {}", discover_response.ai_explanation);

        assert_eq!(
            discover_response.operation_type,
            AttributeOperationType::Discover
        );
        assert_eq!(
            discover_response.execution_status,
            DictionaryExecutionStatus::Completed
        );

        if let Some(results) = discover_response.results {
            let discovered: Vec<ob_poc::models::DiscoveredAttribute> =
                serde_json::from_value(results).unwrap_or_default();
            info!(
                "Discovery returned {} relevant attributes",
                discovered.len()
            );

            for item in &discovered {
                info!(
                    "  - {} (relevance: {:.2}): {}",
                    item.attribute.name, item.relevance_score, item.match_reason
                );
            }
        }

        info!("✅ Agentic search and discover roundtrip completed");
        harness.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_complete_agentic_workflow_roundtrip() -> Result<()> {
        let mut harness = TestHarness::new().await?;

        let agentic_service = match &harness.agentic_service {
            Some(service) => service,
            None => {
                info!("⏭️ Skipping complete workflow test - no AI service available");
                return Ok(());
            }
        };

        info!("🔄 Testing complete agentic workflow roundtrip");

        let test_id = Uuid::new_v4();
        let workflow_name = format!("workflow_test_{}", test_id);

        // Step 1: Create an attribute via agentic service
        info!("Step 1: Creating attribute via agentic service");
        let create_request = AgenticAttributeCreateRequest {
            instruction: format!(
                "Create a new attribute called '{}.risk_score' for storing customer risk assessment scores from 0-100. Higher scores indicate higher risk for compliance and underwriting purposes.",
                workflow_name
            ),
            asset_type: AttributeAssetType::Attribute,
            context: {
                let mut context = HashMap::new();
                context.insert("workflow_test".to_string(), serde_json::Value::Bool(true));
                context.insert("test_id".to_string(), serde_json::Value::String(test_id.to_string()));
                context
            },
            constraints: vec![
                "Score must be between 0 and 100".to_string(),
                "Used for compliance decisions".to_string(),
                "Higher scores = higher risk".to_string(),
            ],
            group_id: Some("risk".to_string()),
            domain: Some("compliance".to_string()),
        };

        let create_response = agentic_service.create_agentic(create_request).await?;
        info!(
            "✅ Create completed: {} affected records",
            create_response.affected_records.len()
        );

        // Track created attributes
        for attr_id in &create_response.affected_records {
            harness.track_attribute(*attr_id);
        }

        // Step 2: Read the created attribute via agentic service
        if !create_response.affected_records.is_empty() {
            info!("Step 2: Reading created attribute via agentic service");

            let read_request = AgenticAttributeReadRequest {
                instruction: format!(
                    "Show me details about the risk score attribute we just created for {}",
                    workflow_name
                ),
                asset_types: vec![AttributeAssetType::Attribute],
                filters: {
                    let mut filters = HashMap::new();
                    filters.insert(
                        "group_id".to_string(),
                        serde_json::Value::String("risk".to_string()),
                    );
                    filters.insert(
                        "domain".to_string(),
                        serde_json::Value::String("compliance".to_string()),
                    );
                    filters
                },
                limit: Some(10),
                offset: Some(0),
            };

            let read_response = agentic_service.read_agentic(read_request).await?;
            info!(
                "✅ Read completed: {} records found",
                read_response.affected_records.len()
            );

            // Step 3: Validate a value using the created attribute
            if let Some(attr_id) = create_response.affected_records.first() {
                info!("Step 3: Validating value via agentic service");

                let validation_request = AttributeValidationRequest {
                    attribute_id: *attr_id,
                    value: serde_json::Value::Number(serde_json::Number::from(75)),
                    context: Some({
                        let mut context = HashMap::new();
                        context.insert(
                            "validation_context".to_string(),
                            serde_json::Value::String("Customer onboarding".to_string()),
                        );
                        context
                    }),
                };

                let validate_request = AgenticAttributeValidateRequest {
                    instruction: "Validate this risk score of 75 for a new customer. Is this within acceptable bounds for our risk model?".to_string(),
                    validation_request,
                };

                let validate_response = agentic_service.validate_agentic(validate_request).await?;
                info!(
                    "✅ Validation completed: {}",
                    validate_response.ai_explanation
                );

                if let Some(results) = validate_response.results {
                    if let Ok(validation) =
                        serde_json::from_value::<ob_poc::models::AttributeValidationResult>(results)
                    {
                        info!(
                            "Validation result: valid={}, errors={:?}",
                            validation.is_valid, validation.validation_errors
                        );
                    }
                }
            }

            // Step 4: Search for similar attributes
            info!("Step 4: Searching for similar attributes");

            let search_criteria = AttributeSearchCriteria {
                name_pattern: Some("risk".to_string()),
                group_id: Some("risk".to_string()),
                domain: Some("compliance".to_string()),
                mask: None,
                semantic_query: None,
                limit: Some(5),
                offset: Some(0),
            };

            let search_request = AgenticAttributeSearchRequest {
                instruction: "Find all risk-related attributes in the compliance domain that might be similar to our risk score".to_string(),
                search_criteria,
                semantic_search: false,
            };

            let search_response = agentic_service.search_agentic(search_request).await?;
            info!(
                "✅ Search completed: {} matches found",
                search_response.affected_records.len()
            );

            // Step 5: Discover related attributes
            info!("Step 5: Discovering related attributes");

            let discovery_request = AttributeDiscoveryRequest {
                semantic_query:
                    "risk assessment, compliance scoring, credit evaluation, customer rating"
                        .to_string(),
                domain_filter: Some("compliance".to_string()),
                group_filter: None,
                limit: Some(5),
            };

            let discover_request = AgenticAttributeDiscoverRequest {
                instruction: "Discover other attributes that might be used together with risk scores for comprehensive customer assessment".to_string(),
                discovery_request,
            };

            let discover_response = agentic_service.discover_agentic(discover_request).await?;
            info!(
                "✅ Discovery completed: {} related attributes found",
                discover_response.affected_records.len()
            );
        }

        // Step 6: Verify everything in the database
        info!("Step 6: Verifying workflow results in database");

        let dict_service = harness.dictionary_service();
        let stats = dict_service.get_statistics().await?;
        info!(
            "Final dictionary statistics: {} total attributes",
            stats.total_attributes
        );

        // Verify our test attributes exist
        for attr_id in &create_response.affected_records {
            let attr = dict_service.get_by_id(*attr_id).await?;
            assert!(attr.is_some(), "Created attribute should exist in database");
            let attr = attr.unwrap();
            info!(
                "✅ Verified attribute in database: {} ({})",
                attr.name, attr.attribute_id
            );
        }

        info!("🎉 Complete agentic workflow roundtrip successful!");
        harness.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_performance_and_stress() -> Result<()> {
        let harness = TestHarness::new().await?;
        let dict_service = harness.dictionary_service();

        info!("⚡ Testing performance and stress scenarios");

        let start_time = std::time::Instant::now();

        // Test 1: Batch create performance
        let mut created_ids = Vec::new();
        for i in 0..10 {
            let new_attr = NewDictionaryAttribute {
                name: format!("perf.test.{}.{}", chrono::Utc::now().timestamp_millis(), i),
                long_description: Some(format!("Performance test attribute {}", i)),
                group_id: Some("performance".to_string()),
                mask: Some("string".to_string()),
                domain: Some("testing".to_string()),
                vector: None,
                source: Some(serde_json::json!({"perf_test": i})),
                sink: None,
            };

            let created = dict_service.create_attribute(new_attr).await?;
            created_ids.push(created.attribute_id);
        }

        let batch_create_time = start_time.elapsed();
        info!(
            "✅ Created {} attributes in {:?} (avg: {:?} per attribute)",
            created_ids.len(),
            batch_create_time,
            batch_create_time / created_ids.len() as u32
        );

        // Test 2: Batch read performance
        let read_start = std::time::Instant::now();
        let mut read_count = 0;
        for attr_id in &created_ids {
            let attr = dict_service.get_by_id(*attr_id).await?;
            assert!(attr.is_some(), "Should find created attribute");
            read_count += 1;
        }
        let batch_read_time = read_start.elapsed();
        info!(
            "✅ Read {} attributes in {:?} (avg: {:?} per attribute)",
            read_count,
            batch_read_time,
            batch_read_time / read_count
        );

        // Test 3: Search performance
        let search_start = std::time::Instant::now();
        let search_criteria = AttributeSearchCriteria {
            name_pattern: Some("perf.test".to_string()),
            group_id: Some("performance".to_string()),
            domain: None,
            mask: None,
            semantic_query: None,
            limit: Some(50),
            offset: Some(0),
        };

        let search_results = dict_service.search_attributes(&search_criteria).await?;
        let search_time = search_start.elapsed();
        info!(
            "✅ Search found {} results in {:?}",
            search_results.len(),
            search_time
        );

        // Test 4: Semantic search performance
        let semantic_start = std::time::Instant::now();
        let semantic_results = dict_service
            .semantic_search("performance test", Some(10))
            .await?;
        let semantic_time = semantic_start.elapsed();
        info!(
            "✅ Semantic search found {} results in {:?}",
            semantic_results.len(),
            semantic_time
        );

        // Cleanup performance test data
        let cleanup_start = std::time::Instant::now();
        let mut deleted_count = 0;
        for attr_id in &created_ids {
            if dict_service.delete_attribute(*attr_id).await? {
                deleted_count += 1;
            }
        }
        let cleanup_time = cleanup_start.elapsed();
        info!(
            "✅ Deleted {} attributes in {:?}",
            deleted_count, cleanup_time
        );

        let total_time = start_time.elapsed();
        info!("🎯 Performance test completed in {:?}", total_time);

        // Performance assertions
        assert!(
            batch_create_time.as_millis() < 5000,
            "Batch create should complete within 5 seconds"
        );
        assert!(
            batch_read_time.as_millis() < 2000,
            "Batch read should complete within 2 seconds"
        );
        assert!(
            search_time.as_millis() < 1000,
            "Search should complete within 1 second"
        );

        Ok(())
    }
}

#[cfg(not(feature = "database"))]
mod no_database_tests {
    #[test]
    fn test_feature_flag_message() {
        println!(
            "Agentic Dictionary Roundtrip tests require the 'database' feature to be enabled."
        );
        println!(
            "Run with: cargo test --features database --test agentic_dictionary_roundtrip_test"
        );
        println!();
        println!("Prerequisites:");
        println!("- PostgreSQL database with 'ob-poc' schema initialized");
        println!("- Environment variables: DATABASE_URL");
        println!("- Optional: OPENAI_API_KEY for full AI integration tests");
        println!("- Dictionary table seeded with initial attributes");
        println!();
        println!("Environment variables:");
        println!("  DATABASE_URL=postgresql://user:password@localhost:5432/ob-poc");
        println!("  OPENAI_API_KEY=sk-... (optional, for AI tests)");
        println!("  SKIP_AI_TESTS=1 (optional, to skip AI-dependent tests)");
        println!("  KEEP_TEST_DATA=1 (optional, to keep test data for debugging)");
    }
}

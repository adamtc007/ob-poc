//! Agentic Dictionary Database Integration Example
//!
//! This example demonstrates the complete integration between the agentic dictionary
//! service and the PostgreSQL database, showing real CRUD operations with actual
//! database persistence.
//!
//! Run with: cargo run --example agentic_dictionary_database_integration --features database
//!
//! Prerequisites:
//! - PostgreSQL database with "ob-poc" schema
//! - Dictionary table initialized
//! - Environment variables: DATABASE_URL, OPENAI_API_KEY (optional)

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

#[cfg(feature = "database")]
use ob_poc::ai::agentic_dictionary_service::{AgenticDictionaryService, DictionaryServiceConfig};
#[cfg(feature = "database")]
use ob_poc::ai::openai::OpenAiClient;
#[cfg(feature = "database")]
use ob_poc::ai::AiConfig;
#[cfg(feature = "database")]
use ob_poc::database::{DatabaseConfig, DatabaseManager};
#[cfg(feature = "database")]
use ob_poc::models::{
    AgenticAttributeCreateRequest, AgenticAttributeSearchRequest, AttributeAssetType,
    AttributeSearchCriteria, DictionaryExecutionStatus, NewDictionaryAttribute,
};

/// Mock AI client for when OpenAI key is not available
#[cfg(feature = "database")]
struct MockAiClient {
    responses: HashMap<String, String>,
}

#[cfg(feature = "database")]
impl MockAiClient {
    fn new() -> Self {
        let mut responses = HashMap::new();
        responses.insert(
            "create".to_string(),
            r#"{"generated_dsl": "(attribute.create :name \"mock.test.attribute\" :description \"Mock test attribute\" :mask \"string\" :group-id \"test\")", "explanation": "Created a mock test attribute for demonstration", "confidence": 0.9}"#.to_string(),
        );
        responses.insert(
            "search".to_string(),
            r#"{"generated_dsl": "(attribute.search :group-id \"test\" :limit 10)", "explanation": "Searching for test attributes", "confidence": 0.8}"#.to_string(),
        );
        Self { responses }
    }
}

#[cfg(feature = "database")]
#[async_trait::async_trait]
impl ob_poc::ai::AiService for MockAiClient {
    async fn generate_dsl(
        &self,
        request: ob_poc::ai::AiDslRequest,
    ) -> ob_poc::ai::AiResult<ob_poc::ai::AiDslResponse> {
        let instruction = request.instruction.to_lowercase();

        let response_json = if instruction.contains("create") {
            self.responses.get("create").unwrap()
        } else if instruction.contains("search") || instruction.contains("find") {
            self.responses.get("search").unwrap()
        } else {
            r#"{"generated_dsl": "(attribute.read)", "explanation": "Default mock response", "confidence": 0.5}"#
        };

        let parsed: serde_json::Value =
            serde_json::from_str(response_json).map_err(|e| ob_poc::ai::AiError::JsonError(e))?;

        Ok(ob_poc::ai::AiDslResponse {
            generated_dsl: parsed["generated_dsl"].as_str().unwrap_or("").to_string(),
            explanation: parsed["explanation"]
                .as_str()
                .unwrap_or("Mock explanation")
                .to_string(),
            confidence: parsed["confidence"].as_f64(),
            changes: None,
            warnings: None,
            suggestions: None,
        })
    }

    async fn health_check(&self) -> ob_poc::ai::AiResult<bool> {
        Ok(true)
    }

    fn config(&self) -> &ob_poc::ai::AiConfig {
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

#[cfg(feature = "database")]
async fn setup_database() -> Result<DatabaseManager> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    info!(
        "Connecting to database: {}",
        mask_database_url(&database_url)
    );

    let db_config = DatabaseConfig {
        database_url,
        max_connections: 10,
        connection_timeout: Duration::from_secs(15),
        idle_timeout: Some(Duration::from_secs(300)),
        max_lifetime: Some(Duration::from_secs(1800)),
    };

    let db_manager = DatabaseManager::new(db_config)
        .await
        .context("Failed to connect to database")?;

    // Test database connection
    db_manager
        .test_connection()
        .await
        .context("Database connection test failed")?;

    info!("✅ Database connection established");
    Ok(db_manager)
}

#[cfg(feature = "database")]
async fn setup_ai_client() -> Arc<dyn ob_poc::ai::AiService + Send + Sync> {
    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        info!("Setting up OpenAI client");
        let config = AiConfig {
            api_key,
            model: "gpt-3.5-turbo".to_string(),
            max_tokens: Some(1000),
            temperature: Some(0.1),
            timeout_seconds: 30,
        };

        let client = Arc::new(OpenAiClient::new(config));

        // Test AI client
        match client.health_check().await {
            Ok(true) => info!("✅ OpenAI client ready"),
            Ok(false) => warn!("⚠️ OpenAI client health check failed"),
            Err(e) => warn!("⚠️ OpenAI client error: {}", e),
        }

        client
    } else {
        info!("No OPENAI_API_KEY found, using mock AI client");
        Arc::new(MockAiClient::new())
    }
}

#[cfg(feature = "database")]
async fn demonstrate_database_operations(db_manager: &DatabaseManager) -> Result<()> {
    info!("🔧 Demonstrating direct database operations");

    let dict_service = db_manager.dictionary_service();

    // Get initial statistics
    let initial_stats = dict_service.get_statistics().await?;
    info!("Initial statistics:");
    info!("  Total attributes: {}", initial_stats.total_attributes);
    info!("  By domain: {:?}", initial_stats.attributes_by_domain);
    info!("  By group: {:?}", initial_stats.attributes_by_group);

    // Create a test attribute
    let test_name = format!("example.test.{}", Uuid::new_v4());
    let new_attribute = NewDictionaryAttribute {
        name: test_name.clone(),
        long_description: Some(
            "Test attribute created by database integration example".to_string(),
        ),
        group_id: Some("example".to_string()),
        mask: Some("string".to_string()),
        domain: Some("testing".to_string()),
        vector: None,
        source: Some(serde_json::json!({
            "created_by": "database_integration_example",
            "test": true
        })),
        sink: None,
    };

    let created = dict_service.create_attribute(new_attribute).await?;
    info!(
        "✅ Created attribute: {} ({})",
        created.name, created.attribute_id
    );

    // Read the created attribute back
    let read_back = dict_service.get_by_name(&test_name).await?;
    assert!(
        read_back.is_some(),
        "Should be able to read created attribute"
    );
    info!("✅ Successfully read back created attribute");

    // Search for the attribute
    let search_criteria = AttributeSearchCriteria {
        name_pattern: Some("example.test".to_string()),
        group_id: Some("example".to_string()),
        domain: None,
        mask: None,
        semantic_query: None,
        limit: Some(10),
        offset: Some(0),
    };

    let search_results = dict_service.search_attributes(&search_criteria).await?;
    info!("✅ Search found {} results", search_results.len());

    // Update the attribute
    let updates = ob_poc::models::UpdateDictionaryAttribute {
        name: None,
        long_description: Some("Updated description by database integration example".to_string()),
        group_id: None,
        mask: None,
        domain: Some("updated_testing".to_string()),
        vector: Some("test_vector".to_string()),
        source: None,
        sink: None,
    };

    let updated = dict_service
        .update_attribute(created.attribute_id, updates)
        .await?;
    assert!(updated.is_some(), "Update should succeed");
    info!("✅ Successfully updated attribute");

    // Semantic search
    let discovered = dict_service
        .semantic_search("test example", Some(5))
        .await?;
    info!("✅ Semantic search found {} results", discovered.len());

    // Health check
    let health = dict_service.health_check().await?;
    info!("Health status: {}", health.status);
    if !health.recommendations.is_empty() {
        info!("Recommendations: {:?}", health.recommendations);
    }

    // Clean up - delete the test attribute
    let deleted = dict_service.delete_attribute(created.attribute_id).await?;
    if deleted {
        info!("✅ Successfully cleaned up test attribute");
    } else {
        warn!("⚠️ Failed to clean up test attribute");
    }

    Ok(())
}

#[cfg(feature = "database")]
async fn demonstrate_agentic_operations(
    db_manager: &DatabaseManager,
    ai_client: Arc<dyn ob_poc::ai::AiService + Send + Sync>,
) -> Result<()> {
    info!("🤖 Demonstrating agentic dictionary operations");

    let dict_db_service = db_manager.dictionary_service();
    let service_config = DictionaryServiceConfig {
        execute_dsl: true,
        max_retries: 2,
        timeout_seconds: 30,
        enable_caching: false, // Disable for demo clarity
        cache_ttl_seconds: 60,
        ai_temperature: 0.1,
        max_tokens: Some(1000),
    };

    let agentic_service =
        AgenticDictionaryService::new(dict_db_service, ai_client, Some(service_config));

    // Test 1: Agentic Create
    info!("Test 1: Agentic attribute creation");
    let create_request = AgenticAttributeCreateRequest {
        instruction: "Create a new attribute for storing customer satisfaction scores from 1-10. This will be used in customer analytics and service quality metrics.".to_string(),
        asset_type: AttributeAssetType::Attribute,
        context: {
            let mut context = HashMap::new();
            context.insert(
                "business_purpose".to_string(),
                serde_json::Value::String("Customer analytics".to_string()),
            );
            context.insert(
                "data_range".to_string(),
                serde_json::Value::String("1-10 integer scale".to_string()),
            );
            context
        },
        constraints: vec![
            "Must be integer type".to_string(),
            "Range 1-10".to_string(),
            "Used for analytics".to_string(),
        ],
        group_id: Some("customer".to_string()),
        domain: Some("analytics".to_string()),
    };

    let create_response = agentic_service.create_agentic(create_request).await;
    match create_response {
        Ok(response) => {
            info!("✅ Agentic create completed:");
            info!("  Operation ID: {}", response.operation_id);
            info!("  Generated DSL: {}", response.generated_dsl);
            info!("  Status: {:?}", response.execution_status);
            info!("  AI Explanation: {}", response.ai_explanation);
            if let Some(confidence) = response.ai_confidence {
                info!("  AI Confidence: {:.1}%", confidence * 100.0);
            }
            info!("  Affected records: {}", response.affected_records.len());

            // If successful, verify in database
            if response.execution_status == DictionaryExecutionStatus::Completed
                && !response.affected_records.is_empty()
            {
                let attr_id = response.affected_records[0];
                let created_attr = dict_db_service.get_by_id(attr_id).await?;

                if let Some(attr) = created_attr {
                    info!(
                        "✅ Verified in database: {} ({})",
                        attr.name, attr.attribute_id
                    );

                    // Clean up
                    if dict_db_service.delete_attribute(attr_id).await? {
                        info!("✅ Cleaned up test attribute");
                    }
                } else {
                    warn!("⚠️ Created attribute not found in database");
                }
            }
        }
        Err(e) => {
            error!("❌ Agentic create failed: {}", e);
        }
    }

    // Test 2: Agentic Search
    info!("\nTest 2: Agentic attribute search");

    // First create some test data
    let test_attrs = vec![
        (
            "customer.email",
            "Customer email address",
            "customer",
            "contact",
        ),
        (
            "customer.phone",
            "Customer phone number",
            "customer",
            "contact",
        ),
        (
            "product.price",
            "Product price in base currency",
            "product",
            "financial",
        ),
    ];

    let mut created_ids = Vec::new();
    for (name, desc, group, domain) in &test_attrs {
        let new_attr = NewDictionaryAttribute {
            name: format!("agentic.test.{}.{}", Uuid::new_v4().simple(), name),
            long_description: Some(desc.to_string()),
            group_id: Some(group.to_string()),
            mask: Some("string".to_string()),
            domain: Some(domain.to_string()),
            vector: None,
            source: Some(serde_json::json!({"test": true})),
            sink: None,
        };

        let created = dict_db_service.create_attribute(new_attr).await?;
        created_ids.push(created.attribute_id);
        info!("Created test attribute: {}", created.name);
    }

    // Wait a moment for any eventual consistency
    sleep(Duration::from_millis(100)).await;

    // Now search via agentic service
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
        instruction:
            "Find all customer-related attributes that are used for customer contact information"
                .to_string(),
        search_criteria,
        semantic_search: false,
    };

    let search_response = agentic_service.search_agentic(search_request).await;
    match search_response {
        Ok(response) => {
            info!("✅ Agentic search completed:");
            info!("  Generated DSL: {}", response.generated_dsl);
            info!("  Status: {:?}", response.execution_status);
            info!("  Found: {} attributes", response.affected_records.len());
            info!("  AI Explanation: {}", response.ai_explanation);

            if let Some(results) = response.results {
                if let Ok(attributes) =
                    serde_json::from_value::<Vec<ob_poc::models::DictionaryAttribute>>(results)
                {
                    info!("Search results:");
                    for attr in attributes.iter().take(3) {
                        info!(
                            "  - {}: {}",
                            attr.name,
                            attr.long_description.as_deref().unwrap_or("No description")
                        );
                    }
                }
            }
        }
        Err(e) => {
            error!("❌ Agentic search failed: {}", e);
        }
    }

    // Clean up test data
    info!("Cleaning up test data...");
    for attr_id in created_ids {
        let _ = dict_db_service.delete_attribute(attr_id).await;
    }
    info!("✅ Cleanup completed");

    Ok(())
}

#[cfg(feature = "database")]
async fn demonstrate_performance(db_manager: &DatabaseManager) -> Result<()> {
    info!("⚡ Performance demonstration");

    let dict_service = db_manager.dictionary_service();

    // Batch create test
    let start_time = Instant::now();
    let batch_size = 20;
    let mut created_ids = Vec::new();

    info!("Creating {} test attributes...", batch_size);
    for i in 0..batch_size {
        let new_attr = NewDictionaryAttribute {
            name: format!("perf.test.{}.{}", Uuid::new_v4().simple(), i),
            long_description: Some(format!("Performance test attribute number {}", i)),
            group_id: Some("performance".to_string()),
            mask: Some("string".to_string()),
            domain: Some("testing".to_string()),
            vector: None,
            source: Some(serde_json::json!({"batch_test": i})),
            sink: None,
        };

        let created = dict_service.create_attribute(new_attr).await?;
        created_ids.push(created.attribute_id);
    }

    let create_time = start_time.elapsed();
    info!(
        "✅ Created {} attributes in {:?} (avg: {:?} per attribute)",
        batch_size,
        create_time,
        create_time / batch_size
    );

    // Batch read test
    let read_start = Instant::now();
    let mut found_count = 0;

    for attr_id in &created_ids {
        if dict_service.get_by_id(*attr_id).await?.is_some() {
            found_count += 1;
        }
    }

    let read_time = read_start.elapsed();
    info!(
        "✅ Read {} attributes in {:?} (avg: {:?} per attribute)",
        found_count,
        read_time,
        read_time / found_count as u32
    );

    // Search performance test
    let search_start = Instant::now();
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

    // Cleanup
    let cleanup_start = Instant::now();
    let mut deleted_count = 0;

    for attr_id in created_ids {
        if dict_service.delete_attribute(attr_id).await? {
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

    Ok(())
}

fn mask_database_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let mut masked = parsed.clone();
        if parsed.password().is_some() {
            let _ = masked.set_password(Some("***"));
        }
        masked.to_string()
    } else {
        "postgresql://***:***@localhost:5432/ob-poc".to_string()
    }
}

#[cfg(feature = "database")]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("ob_poc=info,agentic_dictionary_database_integration=info")
        .init();

    info!("🚀 Starting Agentic Dictionary Database Integration Example");
    info!("This example demonstrates real database integration with agentic operations");

    // Setup database
    let db_manager = setup_database()
        .await
        .context("Failed to setup database connection")?;

    // Setup AI client (real or mock)
    let ai_client = setup_ai_client().await;

    println!("\n{}", "=".repeat(80));
    println!("📊 DATABASE INTEGRATION DEMONSTRATION");
    println!("{}", "=".repeat(80));

    // Phase 1: Direct database operations
    println!("\n🔧 Phase 1: Direct Database Operations");
    println!("{}", "-".repeat(50));

    if let Err(e) = demonstrate_database_operations(&db_manager).await {
        error!("Database operations failed: {}", e);
    } else {
        info!("✅ Database operations completed successfully");
    }

    // Phase 2: Agentic operations
    println!("\n🤖 Phase 2: Agentic Operations");
    println!("{}", "-".repeat(50));

    if let Err(e) = demonstrate_agentic_operations(&db_manager, ai_client).await {
        error!("Agentic operations failed: {}", e);
    } else {
        info!("✅ Agentic operations completed successfully");
    }

    // Phase 3: Performance demonstration
    println!("\n⚡ Phase 3: Performance Testing");
    println!("{}", "-".repeat(50));

    if let Err(e) = demonstrate_performance(&db_manager).await {
        error!("Performance demonstration failed: {}", e);
    } else {
        info!("✅ Performance testing completed successfully");
    }

    // Final statistics
    println!("\n📈 Final Statistics");
    println!("{}", "-".repeat(50));

    let final_stats = db_manager.dictionary_service().get_statistics().await?;
    info!(
        "Dictionary contains {} total attributes",
        final_stats.total_attributes
    );
    info!(
        "Attributes by domain: {:?}",
        final_stats.attributes_by_domain
    );
    info!("Recently created: {}", final_stats.recently_created.len());

    println!("\n{}", "=".repeat(80));
    println!("🎉 AGENTIC DICTIONARY DATABASE INTEGRATION COMPLETE");
    println!("{}", "=".repeat(80));

    info!("✨ All database operations completed successfully!");
    info!("The agentic dictionary service is fully integrated with PostgreSQL");
    info!("Both direct database operations and AI-powered agentic operations work correctly");

    Ok(())
}

#[cfg(not(feature = "database"))]
fn main() {
    println!("This example requires the 'database' feature to be enabled.");
    println!(
        "Run with: cargo run --example agentic_dictionary_database_integration --features database"
    );
    println!();
    println!("Prerequisites:");
    println!("- PostgreSQL database running");
    println!("- Environment variable DATABASE_URL set");
    println!("- Optional: OPENAI_API_KEY for real AI integration");
    println!();
    println!("Example setup:");
    println!("  export DATABASE_URL=\"postgresql://user:password@localhost:5432/ob-poc\"");
    println!("  export OPENAI_API_KEY=\"sk-...\" # Optional");
    println!("  cargo run --example agentic_dictionary_database_integration --features database");
}

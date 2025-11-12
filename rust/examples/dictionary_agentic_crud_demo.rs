//! Dictionary Agentic CRUD Demo
//!
//! This example demonstrates the complete Dictionary Agentic CRUD system:
//! - Natural language instructions converted to DSL
//! - AI-powered attribute management
//! - Database integration with PostgreSQL
//! - Complete CRUD operations for dictionary attributes
//!
//! Run with: cargo run --example dictionary_agentic_crud_demo --features database
//!
//! Prerequisites:
//! - PostgreSQL database with "ob-poc" schema
//! - Environment variables: DATABASE_URL, OPENAI_API_KEY (or GEMINI_API_KEY)
//! - Dictionary table initialized with seed data

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
    AgenticAttributeCreateRequest, AgenticAttributeDeleteRequest, AgenticAttributeDiscoverRequest,
    AgenticAttributeReadRequest, AgenticAttributeSearchRequest, AgenticAttributeUpdateRequest,
    AgenticAttributeValidateRequest, AttributeAssetType, AttributeDiscoveryRequest,
    AttributeSearchCriteria, AttributeValidationRequest, DictionaryExecutionStatus,
    NewDictionaryAttribute,
};

#[cfg(feature = "database")]
use anyhow::{Context, Result};
#[cfg(feature = "database")]
use std::collections::HashMap;
#[cfg(feature = "database")]
use std::sync::Arc;
#[cfg(feature = "database")]
use tracing::{error, info, warn};
#[cfg(feature = "database")]
use uuid::Uuid;

#[cfg(feature = "database")]
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("ob_poc=info,dictionary_agentic_crud_demo=debug")
        .init();

    info!("🚀 Starting Dictionary Agentic CRUD Demo");

    // Check environment variables
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        warn!("DATABASE_URL not set, using default");
        "postgresql://localhost:5432/ob-poc".to_string()
    });

    let openai_api_key = std::env::var("OPENAI_API_KEY").ok();
    let gemini_api_key = std::env::var("GEMINI_API_KEY").ok();

    if openai_api_key.is_none() && gemini_api_key.is_none() {
        error!("Neither OPENAI_API_KEY nor GEMINI_API_KEY is set!");
        error!("Please set one of these environment variables to run the demo.");
        return Ok(());
    }

    // Setup database connection
    info!("📊 Connecting to database...");
    let db_config = DatabaseConfig {
        database_url: database_url.clone(),
        max_connections: 10,
        connection_timeout: std::time::Duration::from_secs(30),
        idle_timeout: Some(std::time::Duration::from_secs(600)),
        max_lifetime: Some(std::time::Duration::from_secs(1800)),
    };

    let db_manager = DatabaseManager::new(db_config)
        .await
        .context("Failed to connect to database")?;

    info!("✅ Database connected successfully");

    // Test database connection
    db_manager
        .test_connection()
        .await
        .context("Database connection test failed")?;

    // Setup AI client
    info!("🤖 Initializing AI client...");
    let ai_client: Arc<dyn ob_poc::ai::AiService + Send + Sync> =
        if let Some(api_key) = openai_api_key {
            info!("Using OpenAI client");
            let config = AiConfig {
                api_key,
                model: "gpt-3.5-turbo".to_string(),
                max_tokens: Some(1000),
                temperature: Some(0.1),
                timeout_seconds: 30,
            };
            Arc::new(OpenAiClient::new(config))
        } else if let Some(api_key) = gemini_api_key {
            info!("Using Gemini client");
            let config = AiConfig {
                api_key,
                model: "gemini-1.5-flash".to_string(),
                max_tokens: Some(1000),
                temperature: Some(0.1),
                timeout_seconds: 30,
            };
            Arc::new(ob_poc::ai::gemini::GeminiClient::new(config))
        } else {
            unreachable!("Should have an API key by now");
        };

    // Test AI client
    match ai_client.health_check().await {
        Ok(true) => info!("✅ AI client health check passed"),
        Ok(false) => warn!("⚠️ AI client health check failed"),
        Err(e) => warn!("⚠️ AI client health check error: {}", e),
    }

    // Setup agentic dictionary service
    info!("⚙️ Setting up Agentic Dictionary Service...");
    let dictionary_db_service = db_manager.dictionary_service();
    let service_config = DictionaryServiceConfig {
        execute_dsl: true,
        max_retries: 3,
        timeout_seconds: 30,
        enable_caching: true,
        cache_ttl_seconds: 300,
        ai_temperature: 0.1,
        max_tokens: Some(1000),
    };

    let agentic_service =
        AgenticDictionaryService::new(dictionary_db_service, ai_client, Some(service_config));

    info!("🎯 Agentic Dictionary Service ready!");

    // Run demo scenarios
    println!("\n" + "=".repeat(60).as_str());
    println!("📋 DICTIONARY AGENTIC CRUD DEMO SCENARIOS");
    println!("=".repeat(60));

    // Scenario 1: Create Attribute
    println!("\n🆕 SCENARIO 1: Create New Attribute");
    println!("-".repeat(40));

    let create_result = demo_create_attribute(&agentic_service).await;
    match create_result {
        Ok(response) => {
            println!("✅ Create operation completed");
            println!("   Generated DSL: {}", response.generated_dsl);
            println!("   Status: {}", response.execution_status);
            println!("   AI Explanation: {}", response.ai_explanation);
            if let Some(confidence) = response.ai_confidence {
                println!("   AI Confidence: {:.1}%", confidence * 100.0);
            }
        }
        Err(e) => println!("❌ Create operation failed: {}", e),
    }

    // Scenario 2: Search Attributes
    println!("\n🔍 SCENARIO 2: Search Existing Attributes");
    println!("-".repeat(40));

    let search_result = demo_search_attributes(&agentic_service).await;
    match search_result {
        Ok(response) => {
            println!("✅ Search operation completed");
            println!("   Generated DSL: {}", response.generated_dsl);
            println!("   Found {} attributes", response.affected_records.len());
            println!("   AI Explanation: {}", response.ai_explanation);

            if let Some(results) = response.results {
                if let Ok(attributes) =
                    serde_json::from_value::<Vec<ob_poc::models::DictionaryAttribute>>(results)
                {
                    for attr in attributes.iter().take(3) {
                        println!(
                            "   - {}: {}",
                            attr.name,
                            attr.long_description.as_deref().unwrap_or("No description")
                        );
                    }
                }
            }
        }
        Err(e) => println!("❌ Search operation failed: {}", e),
    }

    // Scenario 3: Semantic Discovery
    println!("\n🧠 SCENARIO 3: Semantic Attribute Discovery");
    println!("-".repeat(40));

    let discover_result = demo_discover_attributes(&agentic_service).await;
    match discover_result {
        Ok(response) => {
            println!("✅ Discovery operation completed");
            println!("   Generated DSL: {}", response.generated_dsl);
            println!(
                "   Discovered {} attributes",
                response.affected_records.len()
            );
            println!("   AI Explanation: {}", response.ai_explanation);

            if let Some(results) = response.results {
                if let Ok(discovered) =
                    serde_json::from_value::<Vec<ob_poc::models::DiscoveredAttribute>>(results)
                {
                    for item in discovered.iter().take(3) {
                        println!(
                            "   - {}: {} (relevance: {:.2})",
                            item.attribute.name, item.match_reason, item.relevance_score
                        );
                    }
                }
            }
        }
        Err(e) => println!("❌ Discovery operation failed: {}", e),
    }

    // Scenario 4: Attribute Validation
    println!("\n✅ SCENARIO 4: Attribute Value Validation");
    println!("-".repeat(40));

    let validate_result = demo_validate_attribute(&agentic_service).await;
    match validate_result {
        Ok(response) => {
            println!("✅ Validation operation completed");
            println!("   Generated DSL: {}", response.generated_dsl);
            println!("   AI Explanation: {}", response.ai_explanation);

            if let Some(results) = response.results {
                if let Ok(validation) =
                    serde_json::from_value::<ob_poc::models::AttributeValidationResult>(results)
                {
                    println!(
                        "   Validation result: {}",
                        if validation.is_valid {
                            "✅ Valid"
                        } else {
                            "❌ Invalid"
                        }
                    );
                    if !validation.validation_errors.is_empty() {
                        for error in &validation.validation_errors {
                            println!("   Error: {}", error);
                        }
                    }
                }
            }
        }
        Err(e) => println!("❌ Validation operation failed: {}", e),
    }

    // Scenario 5: Read Specific Attributes
    println!("\n📖 SCENARIO 5: Read Specific Attributes");
    println!("-".repeat(40));

    let read_result = demo_read_attributes(&agentic_service).await;
    match read_result {
        Ok(response) => {
            println!("✅ Read operation completed");
            println!("   Generated DSL: {}", response.generated_dsl);
            println!("   Found {} attributes", response.affected_records.len());
            println!("   AI Explanation: {}", response.ai_explanation);
        }
        Err(e) => println!("❌ Read operation failed: {}", e),
    }

    // Performance and Statistics
    println!("\n📊 SCENARIO 6: Service Statistics and Cache Performance");
    println!("-".repeat(40));

    let cache_stats = agentic_service.cache_stats().await;
    println!("Cache Statistics:");
    println!("   Total entries: {}", cache_stats.total_entries);
    println!("   Active entries: {}", cache_stats.active_entries);
    println!("   Expired entries: {}", cache_stats.expired_entries);

    // Get dictionary statistics
    let dict_stats = dictionary_db_service.get_statistics().await;
    match dict_stats {
        Ok(stats) => {
            println!("\nDictionary Statistics:");
            println!("   Total attributes: {}", stats.total_attributes);
            println!("   Attributes by domain: {:?}", stats.attributes_by_domain);
            println!("   Recently created: {}", stats.recently_created.len());
        }
        Err(e) => println!("❌ Failed to get dictionary stats: {}", e),
    }

    // Health Check
    println!("\n🏥 SCENARIO 7: Dictionary Health Check");
    println!("-".repeat(40));

    let health_check = dictionary_db_service.health_check().await;
    match health_check {
        Ok(health) => {
            println!("Dictionary Health Status: {}", health.status);
            println!("   Total attributes: {}", health.total_attributes);
            println!(
                "   Attributes with descriptions: {}/{}",
                health.attributes_with_descriptions, health.total_attributes
            );
            if !health.recommendations.is_empty() {
                println!("   Recommendations:");
                for rec in &health.recommendations {
                    println!("   - {}", rec);
                }
            }
        }
        Err(e) => println!("❌ Health check failed: {}", e),
    }

    // Cleanup
    println!("\n🧹 Cleaning up cache...");
    agentic_service.clear_cache().await;
    println!("✅ Cache cleared");

    println!("\n" + "=".repeat(60).as_str());
    println!("🎉 Dictionary Agentic CRUD Demo completed successfully!");
    println!("=".repeat(60));

    Ok(())
}

#[cfg(feature = "database")]
async fn demo_create_attribute(
    service: &AgenticDictionaryService,
) -> Result<ob_poc::models::AgenticAttributeCrudResponse> {
    let request = AgenticAttributeCreateRequest {
        instruction: "Create a new attribute called 'customer.loyalty_tier' for storing customer loyalty levels like Bronze, Silver, Gold, Platinum. This should be an enumeration type in the CRM domain."
            .to_string(),
        asset_type: AttributeAssetType::Attribute,
        context: {
            let mut context = HashMap::new();
            context.insert(
                "business_context".to_string(),
                serde_json::Value::String("Customer relationship management system".to_string()),
            );
            context.insert(
                "data_sensitivity".to_string(),
                serde_json::Value::String("Medium".to_string()),
            );
            context
        },
        constraints: vec![
            "Must support enum values: Bronze, Silver, Gold, Platinum".to_string(),
            "Default value should be Bronze".to_string(),
            "Required for customer segmentation".to_string(),
        ],
        group_id: Some("customer".to_string()),
        domain: Some("CRM".to_string()),
    };

    service.create_agentic(request).await
}

#[cfg(feature = "database")]
async fn demo_search_attributes(
    service: &AgenticDictionaryService,
) -> Result<ob_poc::models::AgenticAttributeCrudResponse> {
    let criteria = AttributeSearchCriteria {
        name_pattern: Some("customer".to_string()),
        group_id: Some("customer".to_string()),
        domain: Some("CRM".to_string()),
        mask: None,
        semantic_query: None,
        limit: Some(10),
        offset: Some(0),
    };

    let request = AgenticAttributeSearchRequest {
        instruction: "Find all customer-related attributes in the CRM domain that might be used for customer profiling and segmentation"
            .to_string(),
        search_criteria: criteria,
        semantic_search: false,
    };

    service.search_agentic(request).await
}

#[cfg(feature = "database")]
async fn demo_discover_attributes(
    service: &AgenticDictionaryService,
) -> Result<ob_poc::models::AgenticAttributeCrudResponse> {
    let discovery_request = AttributeDiscoveryRequest {
        semantic_query: "financial information, account balance, transaction history, credit score"
            .to_string(),
        domain_filter: Some("finance".to_string()),
        group_filter: None,
        limit: Some(5),
    };

    let request = AgenticAttributeDiscoverRequest {
        instruction: "Discover attributes that would be useful for financial risk assessment and creditworthiness evaluation"
            .to_string(),
        discovery_request,
    };

    service.discover_agentic(request).await
}

#[cfg(feature = "database")]
async fn demo_validate_attribute(
    service: &AgenticDictionaryService,
) -> Result<ob_poc::models::AgenticAttributeCrudResponse> {
    // Use a known attribute ID from the seeded data, or create one
    let test_attribute_id =
        Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap_or_else(|_| Uuid::new_v4());

    let validation_request = AttributeValidationRequest {
        attribute_id: test_attribute_id,
        value: serde_json::Value::String("john.smith@example.com".to_string()),
        context: Some({
            let mut context = HashMap::new();
            context.insert(
                "validation_context".to_string(),
                serde_json::Value::String("Customer onboarding".to_string()),
            );
            context
        }),
    };

    let request = AgenticAttributeValidateRequest {
        instruction: "Validate this email address format for a customer email attribute. Check if it follows proper email format and domain validation rules."
            .to_string(),
        validation_request,
    };

    service.validate_agentic(request).await
}

#[cfg(feature = "database")]
async fn demo_read_attributes(
    service: &AgenticDictionaryService,
) -> Result<ob_poc::models::AgenticAttributeCrudResponse> {
    let request = AgenticAttributeReadRequest {
        instruction: "Show me all attributes in the KYC domain that are used for identity verification and compliance checks"
            .to_string(),
        asset_types: vec![AttributeAssetType::Attribute],
        filters: {
            let mut filters = HashMap::new();
            filters.insert(
                "domain".to_string(),
                serde_json::Value::String("KYC".to_string()),
            );
            filters
        },
        limit: Some(15),
        offset: Some(0),
    };

    service.read_agentic(request).await
}

#[cfg(not(feature = "database"))]
fn main() {
    println!("Dictionary Agentic CRUD Demo requires the 'database' feature.");
    println!("Run with: cargo run --example dictionary_agentic_crud_demo --features database");
    println!();
    println!("Prerequisites:");
    println!("- PostgreSQL database with 'ob-poc' schema");
    println!("- Environment variables: DATABASE_URL, OPENAI_API_KEY (or GEMINI_API_KEY)");
    println!("- Dictionary table initialized with seed data");
}

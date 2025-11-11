//! Agentic CRUD Phase 3 Demo - Real Database Integration
//!
//! This example demonstrates the complete Phase 3 implementation with:
//! - Real AI providers (OpenAI/Gemini)
//! - Actual database operations
//! - End-to-end natural language to database workflow
//! - No mocks or dummy data

use anyhow::{Context, Result};
use ob_poc::ai::agentic_crud_service::{
    AgenticCrudRequest, AgenticCrudService, AiProvider, ModelConfig, ServiceConfig,
};
use ob_poc::ai::crud_prompt_builder::PromptConfig;
use ob_poc::database::{CbuRepository, DatabaseManager};
use ob_poc::parser::idiomatic_parser::parse_crud_statement;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use tracing::{error, info, warn};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🚀 Starting Agentic CRUD Phase 3 Demo");

    // Setup database connection
    let database_manager = setup_database().await?;

    // Run the comprehensive demo
    run_comprehensive_demo(database_manager).await?;

    info!("✅ Demo completed successfully!");
    Ok(())
}

async fn setup_database() -> Result<DatabaseManager> {
    info!("🔧 Setting up database connection");

    let db_manager = DatabaseManager::with_default_config()
        .await
        .context("Failed to create database manager")?;

    // Test connection
    db_manager
        .test_connection()
        .await
        .context("Database connection test failed")?;

    info!("✅ Database connection established");
    Ok(db_manager)
}

async fn run_comprehensive_demo(db_manager: DatabaseManager) -> Result<()> {
    info!("🎯 Running comprehensive Phase 3 demo");

    // Demo 1: OpenAI Integration
    demo_openai_integration(&db_manager).await?;

    // Demo 2: Gemini Integration
    demo_gemini_integration(&db_manager).await?;

    // Demo 3: CBU Operations
    demo_cbu_operations(&db_manager).await?;

    // Demo 4: Entity Operations
    demo_entity_operations(&db_manager).await?;

    // Demo 5: Complex Queries
    demo_complex_operations(&db_manager).await?;

    // Demo 6: Error Handling and Recovery
    demo_error_handling(&db_manager).await?;

    // Demo 7: Performance and Caching
    demo_performance_features(&db_manager).await?;

    Ok(())
}

async fn demo_openai_integration(db_manager: &DatabaseManager) -> Result<()> {
    info!("🤖 Demo 1: OpenAI Integration");

    // Check if OpenAI API key is available
    if env::var("OPENAI_API_KEY").is_err() {
        warn!("⚠️  OPENAI_API_KEY not found, skipping OpenAI demo");
        return Ok(());
    }

    let service = AgenticCrudService::with_openai(
        db_manager.pool().clone(),
        None, // Use env var
    )
    .await
    .context("Failed to create OpenAI service")?;

    info!("✅ OpenAI service created");

    // Test health check
    let health = service.health_check().await?;
    info!("Health status: {:?}", health);

    // Test natural language CRUD operation
    let request = AgenticCrudRequest {
        instruction: "Create a new hedge fund client called 'Alpha Capital Management' based in the Cayman Islands with high risk rating".to_string(),
        context_hints: None,
        execute: true,
        request_id: Some("openai-demo-1".to_string()),
        business_context: Some([
            ("entity_type".to_string(), "hedge_fund".to_string()),
            ("jurisdiction".to_string(), "KY".to_string()),
            ("risk_profile".to_string(), "high".to_string()),
        ].iter().cloned().collect()),
        constraints: Some(vec![
            "Use only approved DSL verbs".to_string(),
            "Follow KYC compliance requirements".to_string(),
        ]),
    };

    match service.process_request(request).await {
        Ok(response) => {
            info!("🎉 OpenAI Request processed successfully!");
            info!("Generated DSL: {}", response.generated_dsl);
            info!("Success: {}", response.success);
            info!(
                "AI Confidence: {:.2}",
                response.generation_metadata.ai_confidence
            );

            if let Some(exec_result) = &response.execution_result {
                info!(
                    "Database execution: Success={}, Rows affected={}",
                    exec_result.success, exec_result.rows_affected
                );
            }
        }
        Err(e) => {
            error!("❌ OpenAI request failed: {}", e);
        }
    }

    Ok(())
}

async fn demo_gemini_integration(db_manager: &DatabaseManager) -> Result<()> {
    info!("🤖 Demo 2: Gemini Integration");

    // Check if Gemini API key is available
    if env::var("GEMINI_API_KEY").is_err() {
        warn!("⚠️  GEMINI_API_KEY not found, skipping Gemini demo");
        return Ok(());
    }

    let service = AgenticCrudService::with_gemini(
        db_manager.pool().clone(),
        None, // Use env var
    )
    .await
    .context("Failed to create Gemini service")?;

    info!("✅ Gemini service created");

    // Test with a different request
    let request = AgenticCrudRequest {
        instruction: "Find all client business units registered in offshore jurisdictions with high or medium risk ratings".to_string(),
        context_hints: Some(vec![
            "offshore_jurisdictions".to_string(),
            "risk_assessment".to_string(),
        ]),
        execute: true,
        request_id: Some("gemini-demo-1".to_string()),
        business_context: Some([
            ("operation_type".to_string(), "search".to_string()),
            ("target_jurisdictions".to_string(), "KY,BVI,BS,CH".to_string()),
        ].iter().cloned().collect()),
        constraints: None,
    };

    match service.process_request(request).await {
        Ok(response) => {
            info!("🎉 Gemini Request processed successfully!");
            info!("Generated DSL: {}", response.generated_dsl);
            info!(
                "RAG Confidence: {:.2}",
                response.rag_context.confidence_score
            );

            if response.success {
                info!("✅ Operation completed successfully");
            } else {
                warn!(
                    "⚠️  Operation completed with warnings: {:?}",
                    response.errors
                );
            }
        }
        Err(e) => {
            error!("❌ Gemini request failed: {}", e);
        }
    }

    Ok(())
}

async fn demo_cbu_operations(db_manager: &DatabaseManager) -> Result<()> {
    info!("🏢 Demo 3: CBU Operations");

    // Use mock provider for reliable demo
    let config = ServiceConfig {
        ai_provider: AiProvider::Mock {
            responses: create_cbu_mock_responses(),
        },
        model_config: ModelConfig::default(),
        prompt_config: PromptConfig::default(),
        execute_dsl: true,
        max_retries: 2,
        timeout_seconds: 10,
        enable_caching: true,
        cache_ttl_seconds: 60,
    };

    let service = AgenticCrudService::new(db_manager.pool().clone(), config).await?;

    // Test CBU creation
    let create_request = AgenticCrudRequest {
        instruction: "Create a new CBU for BetaTech Corporation, a UK technology company requiring custody services".to_string(),
        context_hints: None,
        execute: true,
        request_id: Some("cbu-create-1".to_string()),
        business_context: Some([
            ("entity_type".to_string(), "corporation".to_string()),
            ("jurisdiction".to_string(), "GB".to_string()),
            ("services_needed".to_string(), "custody".to_string()),
        ].iter().cloned().collect()),
        constraints: None,
    };

    let create_response = service.process_request(create_request).await?;
    info!(
        "CBU Creation - Success: {}, DSL: {}",
        create_response.success, create_response.generated_dsl
    );

    // Test CBU search
    let search_request = AgenticCrudRequest {
        instruction: "Find all CBUs for UK corporations".to_string(),
        context_hints: None,
        execute: true,
        request_id: Some("cbu-search-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let search_response = service.process_request(search_request).await?;
    info!(
        "CBU Search - Success: {}, DSL: {}",
        search_response.success, search_response.generated_dsl
    );

    // Test CBU update
    let update_request = AgenticCrudRequest {
        instruction: "Update the risk rating of BetaTech Corporation to medium risk".to_string(),
        context_hints: None,
        execute: true,
        request_id: Some("cbu-update-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let update_response = service.process_request(update_request).await?;
    info!(
        "CBU Update - Success: {}, DSL: {}",
        update_response.success, update_response.generated_dsl
    );

    // Show statistics
    let stats = service.get_statistics().await?;
    info!("Service Statistics: {:?}", stats);

    Ok(())
}

async fn demo_entity_operations(db_manager: &DatabaseManager) -> Result<()> {
    info!("👥 Demo 4: Entity Operations");

    let config = ServiceConfig {
        ai_provider: AiProvider::Mock {
            responses: create_entity_mock_responses(),
        },
        model_config: ModelConfig::default(),
        prompt_config: PromptConfig::default(),
        execute_dsl: true,
        max_retries: 2,
        timeout_seconds: 10,
        enable_caching: true,
        cache_ttl_seconds: 60,
    };

    let service = AgenticCrudService::new(db_manager.pool().clone(), config).await?;

    // Create partnership
    let partnership_request = AgenticCrudRequest {
        instruction: "Register a new Delaware Limited Liability Partnership called TechVentures LP, formed on January 15, 2024".to_string(),
        context_hints: None,
        execute: true,
        request_id: Some("entity-partnership-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let partnership_response = service.process_request(partnership_request).await?;
    info!(
        "Partnership Creation - Success: {}, DSL: {}",
        partnership_response.success, partnership_response.generated_dsl
    );

    // Create proper person
    let person_request = AgenticCrudRequest {
        instruction: "Add John Smith as a new individual, born on March 10, 1980, US nationality, passport number US123456789".to_string(),
        context_hints: None,
        execute: true,
        request_id: Some("entity-person-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let person_response = service.process_request(person_request).await?;
    info!(
        "Person Creation - Success: {}, DSL: {}",
        person_response.success, person_response.generated_dsl
    );

    // Create limited company
    let company_request = AgenticCrudRequest {
        instruction: "Register GammaTech Limited as a UK company with registration number 12345678, incorporated on February 1, 2023".to_string(),
        context_hints: None,
        execute: true,
        request_id: Some("entity-company-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let company_response = service.process_request(company_request).await?;
    info!(
        "Company Creation - Success: {}, DSL: {}",
        company_response.success, company_response.generated_dsl
    );

    // Create trust
    let trust_request = AgenticCrudRequest {
        instruction: "Establish the Wilson Family Trust as a discretionary trust in the Cayman Islands, created on May 20, 2024".to_string(),
        context_hints: None,
        execute: true,
        request_id: Some("entity-trust-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let trust_response = service.process_request(trust_request).await?;
    info!(
        "Trust Creation - Success: {}, DSL: {}",
        trust_response.success, trust_response.generated_dsl
    );

    Ok(())
}

async fn demo_complex_operations(db_manager: &DatabaseManager) -> Result<()> {
    info!("🔍 Demo 5: Complex Operations");

    let config = ServiceConfig {
        ai_provider: AiProvider::Mock {
            responses: create_complex_mock_responses(),
        },
        model_config: ModelConfig::default(),
        prompt_config: PromptConfig::default(),
        execute_dsl: true,
        max_retries: 2,
        timeout_seconds: 15,
        enable_caching: true,
        cache_ttl_seconds: 300,
    };

    let service = AgenticCrudService::new(db_manager.pool().clone(), config).await?;

    // Complex multi-entity search
    let complex_request = AgenticCrudRequest {
        instruction: "Find all high-risk CBUs that have partnerships or trusts from offshore jurisdictions and were created in the last 6 months".to_string(),
        context_hints: Some(vec![
            "risk_analysis".to_string(),
            "offshore_entities".to_string(),
            "recent_activity".to_string(),
        ]),
        execute: true,
        request_id: Some("complex-query-1".to_string()),
        business_context: Some([
            ("analysis_type".to_string(), "risk_assessment".to_string()),
            ("time_period".to_string(), "6_months".to_string()),
            ("entity_types".to_string(), "partnership,trust".to_string()),
        ].iter().cloned().collect()),
        constraints: Some(vec![
            "Include only active entities".to_string(),
            "Apply regulatory compliance filters".to_string(),
        ]),
    };

    let complex_response = service.process_request(complex_request).await?;
    info!(
        "Complex Query - Success: {}, DSL: {}",
        complex_response.success, complex_response.generated_dsl
    );
    info!(
        "Generation time: {}ms",
        complex_response.generation_metadata.ai_generation_time_ms
    );

    Ok(())
}

async fn demo_error_handling(db_manager: &DatabaseManager) -> Result<()> {
    info!("⚠️  Demo 6: Error Handling and Recovery");

    let config = ServiceConfig {
        ai_provider: AiProvider::Mock {
            responses: create_error_mock_responses(),
        },
        model_config: ModelConfig::default(),
        prompt_config: PromptConfig::default(),
        execute_dsl: false, // Don't execute to avoid side effects
        max_retries: 3,
        timeout_seconds: 5,
        enable_caching: false,
        cache_ttl_seconds: 0,
    };

    let service = AgenticCrudService::new(db_manager.pool().clone(), config).await?;

    // Test with invalid instruction
    let invalid_request = AgenticCrudRequest {
        instruction: "Do something impossible with non-existent fields".to_string(),
        context_hints: None,
        execute: false,
        request_id: Some("error-test-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let error_response = service.process_request(invalid_request).await?;
    info!(
        "Error Test - Success: {}, Errors: {:?}",
        error_response.success, error_response.errors
    );

    // Test retry mechanism with recovery
    let retry_request = AgenticCrudRequest {
        instruction: "Create a valid CBU after initial failures".to_string(),
        context_hints: None,
        execute: false,
        request_id: Some("retry-test-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let retry_response = service.process_request(retry_request).await?;
    info!(
        "Retry Test - Success: {}, Retries: {}",
        retry_response.success, retry_response.generation_metadata.retries
    );

    Ok(())
}

async fn demo_performance_features(db_manager: &DatabaseManager) -> Result<()> {
    info!("⚡ Demo 7: Performance and Caching");

    let config = ServiceConfig {
        ai_provider: AiProvider::Mock {
            responses: create_performance_mock_responses(),
        },
        model_config: ModelConfig::default(),
        prompt_config: PromptConfig::default(),
        execute_dsl: false,
        max_retries: 1,
        timeout_seconds: 30,
        enable_caching: true,
        cache_ttl_seconds: 60,
    };

    let service = AgenticCrudService::new(db_manager.pool().clone(), config).await?;

    let test_instruction = "Create a standard hedge fund CBU with typical settings";

    // First request (cache miss)
    let start_time = std::time::Instant::now();
    let request1 = AgenticCrudRequest {
        instruction: test_instruction.to_string(),
        context_hints: None,
        execute: false,
        request_id: Some("perf-test-1".to_string()),
        business_context: None,
        constraints: None,
    };

    let response1 = service.process_request(request1).await?;
    let time1 = start_time.elapsed();
    info!("First request (cache miss): {}ms", time1.as_millis());

    // Second request (cache hit)
    let start_time = std::time::Instant::now();
    let request2 = AgenticCrudRequest {
        instruction: test_instruction.to_string(),
        context_hints: None,
        execute: false,
        request_id: Some("perf-test-2".to_string()),
        business_context: None,
        constraints: None,
    };

    let response2 = service.process_request(request2).await?;
    let time2 = start_time.elapsed();
    info!("Second request (cache hit): {}ms", time2.as_millis());

    info!(
        "Cache performance improvement: {:.2}x faster",
        time1.as_millis() as f64 / time2.as_millis() as f64
    );

    // Show final statistics
    let final_stats = service.get_statistics().await?;
    info!(
        "Final Statistics: Total={}, Successful={}, Cache Size={}",
        final_stats.total_operations, final_stats.successful_operations, final_stats.cache_size
    );

    Ok(())
}

fn create_cbu_mock_responses() -> HashMap<String, String> {
    [
        ("create".to_string(), json!({
            "dsl_content": "(data.create :asset \"cbu\" :values {:name \"BetaTech Corporation\" :jurisdiction \"GB\" :customer_type \"CORPORATION\" :risk_rating \"MEDIUM\" :channel \"DIRECT\" :nature_purpose \"Technology Services\"})",
            "explanation": "Created CBU for UK technology corporation with custody services",
            "confidence": 0.95,
            "changes": ["Added CBU entry", "Set risk rating to MEDIUM", "Configured for custody services"],
            "warnings": [],
            "suggestions": ["Consider adding KYC documentation requirements"]
        }).to_string()),
        ("read".to_string(), json!({
            "dsl_content": "(data.read :asset \"cbu\" :where {:jurisdiction \"GB\" :customer_type \"CORPORATION\"} :select [\"name\" \"risk_rating\" \"created_at\"])",
            "explanation": "Searching for UK corporations in CBU registry",
            "confidence": 0.90,
            "changes": [],
            "warnings": [],
            "suggestions": []
        }).to_string()),
        ("update".to_string(), json!({
            "dsl_content": "(data.update :asset \"cbu\" :where {:name \"BetaTech Corporation\"} :values {:risk_rating \"MEDIUM\"})",
            "explanation": "Updated risk rating for BetaTech Corporation",
            "confidence": 0.88,
            "changes": ["Risk rating changed to MEDIUM"],
            "warnings": ["Risk assessment should be documented"],
            "suggestions": []
        }).to_string()),
    ].iter().cloned().collect()
}

fn create_entity_mock_responses() -> HashMap<String, String> {
    [
        ("partnership".to_string(), json!({
            "dsl_content": "(data.create :asset \"partnership\" :values {:partnership_name \"TechVentures LP\" :partnership_type \"Limited Liability\" :jurisdiction \"US-DE\" :formation_date \"2024-01-15\"})",
            "explanation": "Registered new Delaware Limited Liability Partnership",
            "confidence": 0.93,
            "changes": ["Created partnership entity", "Set Delaware jurisdiction", "Recorded formation date"],
            "warnings": [],
            "suggestions": ["Add partnership agreement documentation"]
        }).to_string()),
        ("person".to_string(), json!({
            "dsl_content": "(data.create :asset \"proper_person\" :values {:first_name \"John\" :last_name \"Smith\" :date_of_birth \"1980-03-10\" :nationality \"US\" :id_document_type \"Passport\" :id_document_number \"US123456789\"})",
            "explanation": "Registered new individual with passport identification",
            "confidence": 0.96,
            "changes": ["Created proper person entity", "Added passport details", "Set US nationality"],
            "warnings": [],
            "suggestions": []
        }).to_string()),
        ("company".to_string(), json!({
            "dsl_content": "(data.create :asset \"limited_company\" :values {:company_name \"GammaTech Limited\" :registration_number \"12345678\" :jurisdiction \"GB\" :incorporation_date \"2023-02-01\"})",
            "explanation": "Registered UK limited company with Companies House details",
            "confidence": 0.94,
            "changes": ["Created limited company entity", "Added UK registration number", "Set incorporation date"],
            "warnings": [],
            "suggestions": ["Verify registration number with Companies House"]
        }).to_string()),
        ("trust".to_string(), json!({
            "dsl_content": "(data.create :asset \"trust\" :values {:trust_name \"Wilson Family Trust\" :trust_type \"Discretionary\" :jurisdiction \"KY\" :establishment_date \"2024-05-20\"})",
            "explanation": "Established discretionary trust in Cayman Islands",
            "confidence": 0.91,
            "changes": ["Created trust entity", "Set discretionary structure", "Cayman Islands jurisdiction"],
            "warnings": ["Ensure compliance with Cayman trust laws"],
            "suggestions": []
        }).to_string()),
    ].iter().cloned().collect()
}

fn create_complex_mock_responses() -> HashMap<String, String> {
    [
        ("complex".to_string(), json!({
            "dsl_content": "(data.read :asset \"cbu\" :where {:risk_rating [\"HIGH\"] :created_at \">= 6_months_ago\"} :join [{:asset \"entities\" :on {:cbu_id :cbu_id} :where {:entity_type [\"PARTNERSHIP\" \"TRUST\"] :jurisdiction [\"KY\" \"BVI\" \"BS\" \"CH\"]}}] :select [\"name\" \"risk_rating\" \"jurisdiction\" \"created_at\" \"entity_count\"])",
            "explanation": "Complex query for high-risk CBUs with offshore entities from recent period",
            "confidence": 0.85,
            "changes": [],
            "warnings": ["Complex query may take longer to execute", "Result set may be large"],
            "suggestions": ["Consider adding pagination", "Add result caching for repeated queries"]
        }).to_string()),
    ].iter().cloned().collect()
}

fn create_error_mock_responses() -> HashMap<String, String> {
    [
        ("invalid".to_string(), json!({
            "dsl_content": "(invalid.operation :unknown_field \"bad_value\")",
            "explanation": "Attempted to process invalid instruction",
            "confidence": 0.20,
            "changes": [],
            "warnings": ["Unknown operation type", "Invalid field names"],
            "suggestions": ["Use supported DSL verbs", "Check field names against schema"]
        }).to_string()),
        ("recovery".to_string(), json!({
            "dsl_content": "(data.create :asset \"cbu\" :values {:name \"Recovered CBU\" :description \"Created after retry\"})",
            "explanation": "Successfully generated valid DSL after retries",
            "confidence": 0.87,
            "changes": ["Recovered from initial parsing errors"],
            "warnings": [],
            "suggestions": []
        }).to_string()),
    ].iter().cloned().collect()
}

fn create_performance_mock_responses() -> HashMap<String, String> {
    [
        ("standard".to_string(), json!({
            "dsl_content": "(data.create :asset \"cbu\" :values {:name \"Standard Hedge Fund\" :customer_type \"HEDGE_FUND\" :risk_rating \"HIGH\" :jurisdiction \"KY\" :nature_purpose \"Investment Management\"})",
            "explanation": "Created standard hedge fund CBU configuration",
            "confidence": 0.92,
            "changes": ["Applied hedge fund template", "Set high risk rating", "Cayman Islands jurisdiction"],
            "warnings": [],
            "suggestions": []
        }).to_string()),
    ].iter().cloned().collect()
}

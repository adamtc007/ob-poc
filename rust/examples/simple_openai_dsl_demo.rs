//! Simple OpenAI DSL Demo - Basic AI Integration Test
//!
//! This example demonstrates a simple OpenAI integration for DSL generation,
//! focusing on the core AI-to-DSL conversion functionality without complex
//! database operations.
//!
//! ## What This Demonstrates:
//! 1. OpenAI API integration
//! 2. Natural language to DSL conversion
//! 3. DSL parsing and validation
//! 4. Basic error handling
//!
//! ## Prerequisites:
//! - Set OPENAI_API_KEY environment variable
//!
//! ## Usage:
//! ```bash
//! export OPENAI_API_KEY="your-openai-api-key"
//! cargo run --example simple_openai_dsl_demo
//! ```

#[cfg(feature = "database")]
use ob_poc::services::ai_dsl_service::{AiDslService, AiOnboardingRequest};
use ob_poc::{dsl_manager::CleanDslManager, parse_dsl};

use std::env;
use std::time::Instant;
use tokio;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Simple OpenAI DSL Demo Starting");
    info!("🤖 Testing basic OpenAI integration for DSL generation");

    let demo_start = Instant::now();

    // Check for OpenAI API key
    let openai_key = env::var("OPENAI_API_KEY");
    if openai_key.is_err() {
        warn!("⚠️  OPENAI_API_KEY not found. Running mock demo instead...");
        run_mock_demo().await?;
        return Ok(());
    }

    info!("✅ OpenAI API key found, proceeding with real AI integration...");

    #[cfg(feature = "database")]
    {
        run_openai_demo().await?;
    }

    #[cfg(not(feature = "database"))]
    {
        info!("💡 Database features not enabled. Running parsing demo only...");
        run_parsing_demo().await?;
    }

    let total_time = demo_start.elapsed();
    info!(
        "🏁 Simple OpenAI DSL Demo completed in {}ms",
        total_time.as_millis()
    );

    Ok(())
}

/// Run the OpenAI demo with full AI service integration
#[cfg(feature = "database")]
async fn run_openai_demo() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Running OpenAI demo with AI service...");

    // Initialize AI service with OpenAI
    let ai_service = AiDslService::new_with_openai(None).await?;
    info!("✅ OpenAI AI service initialized");

    // Simple test request
    let test_request = AiOnboardingRequest {
        instruction: "Create a simple onboarding case for a UK corporation requiring basic KYC and custody services".to_string(),
        client_name: "SimpleCorpDemo Ltd".to_string(),
        jurisdiction: "GB".to_string(),
        entity_type: "CORP".to_string(),
        services: vec!["CUSTODY".to_string(), "KYC".to_string()],
        additional_context: Some("Simple demo client for testing".to_string()),
    };

    info!("📤 Sending request to OpenAI...");
    info!("   Client: {}", test_request.client_name);
    info!("   Instruction: {}", test_request.instruction);

    let request_start = Instant::now();

    match ai_service.create_ai_onboarding(test_request).await {
        Ok(ai_response) => {
            let request_time = request_start.elapsed();

            info!(
                "✅ OpenAI response received in {}ms",
                request_time.as_millis()
            );
            info!("🆔 Generated CBU ID: {}", ai_response.generated_cbu_id);
            info!("📝 DSL Content ({} chars):", ai_response.dsl_content.len());

            // Print the generated DSL with proper formatting
            println!("\n--- Generated DSL ---");
            println!("{}", ai_response.dsl_content);
            println!("--- End DSL ---\n");

            // Validate the generated DSL
            info!("🔍 Validating generated DSL...");
            match parse_dsl(&ai_response.dsl_content) {
                Ok(parsed_program) => {
                    info!(
                        "✅ DSL validation successful: {} statements parsed",
                        parsed_program.len()
                    );

                    // Test processing through DSL manager
                    let mut manager = CleanDslManager::new();
                    let result = manager.process_dsl_request(ai_response.dsl_content).await;

                    if result.success {
                        info!("✅ DSL Manager processing successful");
                    } else {
                        warn!("⚠️  DSL Manager processing had issues: {:?}", result.errors);
                    }
                }
                Err(e) => {
                    error!("❌ DSL validation failed: {}", e);
                }
            }

            // Display execution details if available
            if let Some(details) = ai_response.execution_details {
                info!("📊 Execution details:");
                info!("   Model used: {:?}", details.get("model"));
                info!("   Tokens used: {:?}", details.get("tokens"));
                info!(
                    "   Processing time: {:?}",
                    details.get("processing_time_ms")
                );
            }
        }
        Err(e) => {
            error!("❌ OpenAI request failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

/// Run a parsing-only demo when database features are not available
async fn run_parsing_demo() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Running parsing demo (no AI service, but testing DSL processing)...");

    // Sample DSL that mimics what OpenAI might generate
    let sample_dsl = r#"
(case.create
    :case-id "CBU-GB-DEMO-001"
    :client-name "SimpleCorpDemo Ltd"
    :jurisdiction "GB"
    :entity-type "CORP")

(services.add :case-id "CBU-GB-DEMO-001" :service-type "CUSTODY")

(kyc.start :case-id "CBU-GB-DEMO-001")

(identity.verify
    :case-id "CBU-GB-DEMO-001"
    :document-type "INCORPORATION_CERTIFICATE")

(compliance.screen
    :case-id "CBU-GB-DEMO-001"
    :screen-type "AML")

(case.approve :case-id "CBU-GB-DEMO-001")
"#;

    info!("📝 Testing with sample DSL ({} chars)", sample_dsl.len());
    println!("\n--- Sample DSL ---");
    println!("{}", sample_dsl);
    println!("--- End Sample DSL ---\n");

    // Parse and validate
    info!("🔍 Parsing sample DSL...");
    match parse_dsl(sample_dsl) {
        Ok(parsed_program) => {
            info!(
                "✅ DSL parsing successful: {} statements",
                parsed_program.len()
            );

            // Process through DSL manager
            let mut manager = CleanDslManager::new();
            let result = manager.process_dsl_request(sample_dsl.to_string()).await;

            if result.success {
                info!("✅ DSL Manager processing successful");
                info!("📊 Processing successful");
            } else {
                warn!("⚠️  DSL Manager processing had issues: {:?}", result.errors);
            }
        }
        Err(e) => {
            error!("❌ DSL parsing failed: {}", e);
        }
    }

    Ok(())
}

/// Run a mock demo when no OpenAI API key is available
async fn run_mock_demo() -> Result<(), Box<dyn std::error::Error>> {
    info!("🎭 Running mock demo (no OpenAI API key required)...");

    let mock_scenarios = vec![
        ("SimpleCorpDemo Ltd", "Basic UK corporation onboarding"),
        ("TechStartup Inc", "US technology company setup"),
        ("EuroFund SARL", "European investment fund establishment"),
    ];

    for (client_name, description) in mock_scenarios {
        info!("🏢 Mock scenario: {} - {}", client_name, description);

        // Generate mock CBU ID
        let mock_cbu_id = format!("MOCK-{}", client_name.replace(" ", "-").to_uppercase());

        // Generate mock DSL
        let mock_dsl = format!(
            r#"(case.create :case-id "{}" :client-name "{}")
(kyc.start :case-id "{}")
(services.add :case-id "{}" :service-type "CUSTODY")"#,
            mock_cbu_id, client_name, mock_cbu_id, mock_cbu_id
        );

        info!("🆔 Mock CBU ID: {}", mock_cbu_id);
        info!("📝 Mock DSL: {} characters", mock_dsl.len());

        // Validate mock DSL
        match parse_dsl(&mock_dsl) {
            Ok(program) => {
                info!("✅ Mock DSL is valid: {} statements", program.len());
            }
            Err(e) => {
                warn!("⚠️  Mock DSL validation failed: {}", e);
            }
        }

        info!(""); // Empty line for readability
    }

    info!("💡 To test with real OpenAI, set OPENAI_API_KEY environment variable");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_demo_runs() {
        let result = run_mock_demo().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_parsing_demo_runs() {
        let result = run_parsing_demo().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_sample_dsl_is_valid() {
        let sample_dsl = r#"(case.create :case-id "TEST" :client-name "Test Client")"#;
        let result = parse_dsl(sample_dsl);
        assert!(result.is_ok());
    }
}

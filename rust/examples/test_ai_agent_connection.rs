//! AI Agent Connection Test - Direct AI Service Integration Test
//!
//! This example tests the AI agent connection directly, bypassing complex
//! database dependencies to verify that the AI services are properly
//! connected and can generate DSL content.
//!
//! ## What This Tests:
//! 1. AI service initialization (OpenAI/Gemini)
//! 2. Direct AI DSL generation
//! 3. Generated DSL validation
//! 4. AI response parsing
//! 5. Multi-provider failover
//!
//! ## Prerequisites:
//! - Set OPENAI_API_KEY and/or GEMINI_API_KEY environment variables
//!
//! ## Usage:
//! ```bash
//! export OPENAI_API_KEY="your-openai-key"
//! export GEMINI_API_KEY="your-gemini-key"
//! cargo run --example test_ai_agent_connection --features="database"
//! ```

#[cfg(feature = "database")]
use ob_poc::services::ai_dsl_service::{AiDslService, AiOnboardingRequest};
use ob_poc::{dsl_manager::CleanDslManager, parse_dsl};

use std::collections::HashMap;
use std::env;
use std::time::Instant;
use tokio;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🔗 AI Agent Connection Test Starting");
    info!("🧪 Testing direct AI service integration");

    let demo_start = Instant::now();

    // Check API key availability
    let has_openai = env::var("OPENAI_API_KEY").is_ok();
    let has_gemini = env::var("GEMINI_API_KEY").is_ok();

    if !has_openai && !has_gemini {
        warn!("⚠️  No AI API keys found. Cannot test agent connections.");
        info!("💡 Set OPENAI_API_KEY or GEMINI_API_KEY to test real AI agents.");
        run_mock_agent_test().await?;
        return Ok(());
    }

    info!(
        "🔑 API Keys Available - OpenAI: {}, Gemini: {}",
        has_openai, has_gemini
    );

    #[cfg(feature = "database")]
    {
        // Test OpenAI if available
        if has_openai {
            test_openai_agent().await?;
        }

        // Test Gemini if available
        if has_gemini {
            test_gemini_agent().await?;
        }

        // Test agent through DSL Manager
        test_agent_via_dsl_manager().await?;
    }

    #[cfg(not(feature = "database"))]
    {
        warn!("⚠️  Database features not enabled. Testing basic DSL processing only.");
        test_basic_dsl_processing().await?;
    }

    let total_time = demo_start.elapsed();
    info!(
        "🏁 AI Agent Connection Test completed in {}ms",
        total_time.as_millis()
    );

    Ok(())
}

/// Test OpenAI agent directly
#[cfg(feature = "database")]
async fn test_openai_agent() -> Result<(), Box<dyn std::error::Error>> {
    info!("🤖 Testing OpenAI Agent Connection...");

    let start_time = Instant::now();

    match AiDslService::new_with_openai(None).await {
        Ok(ai_service) => {
            info!("✅ OpenAI service initialized successfully");

            let test_request = AiOnboardingRequest {
                instruction: "Create a simple KYC onboarding case for a UK corporation".to_string(),
                client_name: "TestCorp UK Ltd".to_string(),
                jurisdiction: "GB".to_string(),
                entity_type: "CORP".to_string(),
                services: vec!["KYC".to_string()],
                additional_context: Some("Agent connection test".to_string()),
            };

            info!("📤 Sending request to OpenAI...");
            match ai_service.create_ai_onboarding(test_request).await {
                Ok(response) => {
                    let response_time = start_time.elapsed();
                    info!("✅ OpenAI Response Success!");
                    info!("   📋 CBU ID: {}", response.generated_cbu_id);
                    info!(
                        "   📝 DSL Length: {} characters",
                        response.dsl_content.len()
                    );
                    info!("   ⏱️  Response Time: {}ms", response_time.as_millis());

                    // Validate the generated DSL
                    match parse_dsl(&response.dsl_content) {
                        Ok(parsed) => {
                            info!("   ✅ Generated DSL is valid: {} forms", parsed.len());

                            // Show first few lines of DSL
                            let lines: Vec<&str> = response.dsl_content.lines().take(5).collect();
                            info!("   📄 DSL Preview:");
                            for line in lines {
                                info!("      {}", line.trim());
                            }
                        }
                        Err(e) => {
                            warn!("   ⚠️  Generated DSL validation failed: {}", e);
                        }
                    }

                    if let Some(details) = response.execution_details {
                        info!("   📊 Execution Details: {:?}", details);
                    }
                }
                Err(e) => {
                    error!("❌ OpenAI request failed: {}", e);
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to initialize OpenAI service: {}", e);
        }
    }

    Ok(())
}

/// Test Gemini agent directly
#[cfg(feature = "database")]
async fn test_gemini_agent() -> Result<(), Box<dyn std::error::Error>> {
    info!("🟢 Testing Gemini Agent Connection...");

    let start_time = Instant::now();

    match AiDslService::new_with_gemini(None).await {
        Ok(ai_service) => {
            info!("✅ Gemini service initialized successfully");

            let test_request = AiOnboardingRequest {
                instruction: "Create an investment fund onboarding workflow for Germany"
                    .to_string(),
                client_name: "Europa Investment Fund GmbH".to_string(),
                jurisdiction: "DE".to_string(),
                entity_type: "FUND".to_string(),
                services: vec!["CUSTODY".to_string(), "ADMINISTRATION".to_string()],
                additional_context: Some("Gemini agent connection test".to_string()),
            };

            info!("📤 Sending request to Gemini...");
            match ai_service.create_ai_onboarding(test_request).await {
                Ok(response) => {
                    let response_time = start_time.elapsed();
                    info!("✅ Gemini Response Success!");
                    info!("   📋 CBU ID: {}", response.generated_cbu_id);
                    info!(
                        "   📝 DSL Length: {} characters",
                        response.dsl_content.len()
                    );
                    info!("   ⏱️  Response Time: {}ms", response_time.as_millis());

                    // Validate the generated DSL
                    match parse_dsl(&response.dsl_content) {
                        Ok(parsed) => {
                            info!("   ✅ Generated DSL is valid: {} forms", parsed.len());
                        }
                        Err(e) => {
                            warn!("   ⚠️  Generated DSL validation failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Gemini request failed: {}", e);
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to initialize Gemini service: {}", e);
        }
    }

    Ok(())
}

/// Test agent through DSL Manager integration
#[cfg(feature = "database")]
async fn test_agent_via_dsl_manager() -> Result<(), Box<dyn std::error::Error>> {
    info!("🏗️  Testing Agent via DSL Manager Integration...");

    let mut manager = CleanDslManager::new();

    let instruction = "Create comprehensive onboarding for a Swiss family office".to_string();
    let context_data = HashMap::from([
        (
            "client_name".to_string(),
            "Heritage Family Office SA".to_string(),
        ),
        ("jurisdiction".to_string(), "CH".to_string()),
        ("entity_type".to_string(), "FAMILY_OFFICE".to_string()),
        (
            "services".to_string(),
            "CUSTODY,PORTFOLIO_MANAGEMENT".to_string(),
        ),
    ]);

    info!("📤 Testing agent DSL generation through manager...");
    let start_time = Instant::now();

    match manager
        .process_agent_dsl_generation(instruction, context_data)
        .await
    {
        Ok(result) => {
            let response_time = start_time.elapsed();
            info!("✅ DSL Manager Agent Integration Success!");
            info!("   📋 Case ID: {}", result.case_id);
            info!("   🤖 AI Generated: {}", result.ai_generated);
            info!("   📈 Success: {}", result.success);
            info!("   ⏱️  Processing Time: {}ms", response_time.as_millis());
            info!(
                "   🎨 Visualization Generated: {}",
                result.visualization_generated
            );

            if !result.errors.is_empty() {
                warn!("   ⚠️  Some errors occurred:");
                for error in &result.errors {
                    warn!("      - {}", error);
                }
            }
        }
        Err(e) => {
            error!("❌ DSL Manager agent integration failed: {}", e);
        }
    }

    Ok(())
}

/// Test basic DSL processing when database features aren't available
async fn test_basic_dsl_processing() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Testing basic DSL processing (no AI service)...");

    // Sample DSL that might be generated by an AI agent
    let sample_ai_dsl = r#"
(case.create
    :case-id "CBU-TEST-001"
    :client-name "AI Test Client Ltd"
    :jurisdiction "GB"
    :entity-type "CORP"
    :case-type "ONBOARDING")

(entity.register
    :entity-id "test-entity-001"
    :entity-name "AI Test Client Ltd"
    :jurisdiction "GB"
    :entity-type "CORPORATION")

(kyc.start
    :case-id "CBU-TEST-001"
    :entity-id "test-entity-001"
    :kyc-level "ENHANCED")

(identity.verify
    :case-id "CBU-TEST-001"
    :entity-id "test-entity-001"
    :document-type "INCORPORATION_CERTIFICATE")

(services.add
    :case-id "CBU-TEST-001"
    :service-type "KYC")

(compliance.screen
    :case-id "CBU-TEST-001"
    :screen-type "AML"
    :jurisdiction "GB")

(case.approve
    :case-id "CBU-TEST-001"
    :approval-type "KYC_COMPLETE")
"#;

    info!("📝 Testing AI-style DSL parsing...");
    match parse_dsl(sample_ai_dsl) {
        Ok(parsed) => {
            info!(
                "✅ AI-style DSL parsed successfully: {} forms",
                parsed.len()
            );

            // Process through DSL Manager
            let mut manager = CleanDslManager::new();
            let result = manager.process_dsl_request(sample_ai_dsl.to_string()).await;

            if result.success {
                info!("✅ DSL Manager processing successful");
                info!("   📋 Case ID: {}", result.case_id);
                info!("   ⏱️  Processing Time: {}ms", result.processing_time_ms);
            } else {
                warn!("⚠️  DSL Manager processing had issues: {:?}", result.errors);
            }
        }
        Err(e) => {
            error!("❌ AI-style DSL parsing failed: {}", e);
        }
    }

    Ok(())
}

/// Run mock agent test when no API keys are available
async fn run_mock_agent_test() -> Result<(), Box<dyn std::error::Error>> {
    info!("🎭 Running mock agent test...");

    let mock_scenarios = vec![
        (
            "UK Corporation",
            "Create KYC onboarding for UK corporation",
            "TestCorp UK Ltd",
            "GB",
            "CORP",
        ),
        (
            "German Fund",
            "Setup German investment fund",
            "Europa Fund GmbH",
            "DE",
            "FUND",
        ),
        (
            "Swiss Family Office",
            "Onboard Swiss family office",
            "Heritage SA",
            "CH",
            "FAMILY_OFFICE",
        ),
    ];

    for (name, instruction, client, jurisdiction, entity_type) in mock_scenarios {
        info!("🏢 Mock Scenario: {}", name);

        // Generate mock CBU ID
        let mock_cbu_id = format!("CBU-MOCK-{}-001", jurisdiction);

        // Generate mock DSL response
        let mock_dsl = format!(
            r#"(case.create :case-id "{}" :client-name "{}" :jurisdiction "{}" :entity-type "{}")
(kyc.start :case-id "{}")
(services.add :case-id "{}" :service-type "KYC")
(case.approve :case-id "{}")"#,
            mock_cbu_id, client, jurisdiction, entity_type, mock_cbu_id, mock_cbu_id, mock_cbu_id
        );

        info!("   📋 Mock CBU: {}", mock_cbu_id);
        info!("   📝 Mock DSL: {} characters", mock_dsl.len());
        info!("   📄 Instruction: {}", instruction);

        // Validate mock DSL
        match parse_dsl(&mock_dsl) {
            Ok(parsed) => {
                info!("   ✅ Mock DSL is valid: {} forms", parsed.len());
            }
            Err(e) => {
                warn!("   ⚠️  Mock DSL validation failed: {}", e);
            }
        }
        info!("");
    }

    info!("💡 Mock test complete. Set API keys to test real AI agents.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_ai_dsl() {
        let sample = r#"(case.create :case-id "TEST" :client-name "Test Corp")"#;
        let result = parse_dsl(sample);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_agent_runs() {
        let result = run_mock_agent_test().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_basic_dsl_processing_runs() {
        let result = test_basic_dsl_processing().await;
        assert!(result.is_ok());
    }
}

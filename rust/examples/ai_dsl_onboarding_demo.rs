//! AI DSL Onboarding Demo - Full AI Workflow Integration
//!
//! This example demonstrates the complete AI-powered DSL onboarding workflow,
//! showcasing natural language to DSL conversion, validation, and execution.
//!
//! ## What This Demonstrates:
//! 1. AI Service initialization with multiple providers
//! 2. Natural language to DSL conversion
//! 3. CBU (Client Business Unit) generation
//! 4. DSL validation and parsing
//! 5. End-to-end onboarding workflow
//!
//! ## Prerequisites:
//! - Set OPENAI_API_KEY or GEMINI_API_KEY environment variable
//! - Optional: DATABASE_URL for database integration
//!
//! ## Usage:
//! ```bash
//! export OPENAI_API_KEY="your-openai-api-key"
//! cargo run --example ai_dsl_onboarding_demo
//! ```

#[cfg(feature = "database")]
use ob_poc::services::{
    ai_dsl_service::{AiDslService, AiOnboardingRequest},
    dsl_lifecycle::{DslChangeRequest, DslChangeType, DslLifecycleService},
};
use ob_poc::{
    dsl_manager::CleanDslManager,
    dsl_visualizer::{DslVisualizer, StateResult},
    parse_dsl,
};

use std::collections::HashMap;
use std::env;
use std::time::Instant;
use tokio;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 AI DSL Onboarding Demo Starting");
    info!("🤖 Testing full AI-powered onboarding workflow");

    let demo_start = Instant::now();

    // Step 1: Check for API keys
    let has_openai = env::var("OPENAI_API_KEY").is_ok();
    let has_gemini = env::var("GEMINI_API_KEY").is_ok();
    let has_database = env::var("DATABASE_URL").is_ok();

    if !has_openai && !has_gemini {
        warn!("⚠️  No AI API keys found. Set OPENAI_API_KEY or GEMINI_API_KEY to test full functionality.");
        info!("🔄 Running mock AI demo instead...");
        run_mock_ai_demo().await?;
        return Ok(());
    }

    info!(
        "✅ Found API keys - OpenAI: {}, Gemini: {}",
        has_openai, has_gemini
    );
    info!("💾 Database available: {}", has_database);

    #[cfg(feature = "database")]
    if has_database {
        // Full AI workflow with database
        run_full_ai_workflow().await?;
    } else {
        // AI workflow without database
        run_ai_workflow_no_db().await?;
    }

    #[cfg(not(feature = "database"))]
    {
        run_ai_workflow_no_db().await?;
    }

    let total_time = demo_start.elapsed();
    info!(
        "🏁 AI DSL Onboarding Demo completed in {}ms",
        total_time.as_millis()
    );

    Ok(())
}

/// Run the full AI workflow with database integration
#[cfg(feature = "database")]
async fn run_full_ai_workflow() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Running full AI workflow with database integration...");

    // Step 1: Initialize AI service
    let ai_service = if env::var("OPENAI_API_KEY").is_ok() {
        AiDslService::new_with_openai(None).await?
    } else {
        AiDslService::new_with_gemini(None).await?
    };

    // Step 2: Initialize lifecycle service
    let lifecycle_service = ob_poc::services::create_lifecycle_service().await?;

    // Step 3: Test various onboarding scenarios
    let test_scenarios = vec![
        AiOnboardingRequest {
            instruction: "Create onboarding for a UK tech startup requiring custody services"
                .to_string(),
            client_name: "TechCorp Ltd".to_string(),
            jurisdiction: "GB".to_string(),
            entity_type: "CORP".to_string(),
            services: vec!["CUSTODY".to_string()],
            additional_context: Some("Fast-growing fintech company".to_string()),
        },
        AiOnboardingRequest {
            instruction: "Set up German hedge fund with UCITS compliance".to_string(),
            client_name: "Europa Capital GmbH".to_string(),
            jurisdiction: "DE".to_string(),
            entity_type: "FUND".to_string(),
            services: vec!["CUSTODY".to_string(), "ADMINISTRATION".to_string()],
            additional_context: Some("UCITS compliant fund structure".to_string()),
        },
        AiOnboardingRequest {
            instruction: "Onboard US investment manager for derivatives trading".to_string(),
            client_name: "Alpha Investments LLC".to_string(),
            jurisdiction: "US".to_string(),
            entity_type: "INVESTMENT_MANAGER".to_string(),
            services: vec!["PRIME_SERVICES".to_string(), "DERIVATIVES".to_string()],
            additional_context: Some("Focus on equity derivatives".to_string()),
        },
    ];

    for (i, scenario) in test_scenarios.iter().enumerate() {
        info!("📋 Testing scenario {}: {}", i + 1, scenario.client_name);

        let start_time = Instant::now();

        // Generate AI onboarding
        match ai_service.create_ai_onboarding(scenario.clone()).await {
            Ok(ai_response) => {
                info!("✅ AI Response: CBU {}", ai_response.generated_cbu_id);
                info!("📝 Generated DSL: {} chars", ai_response.dsl_content.len());

                // Process through lifecycle service
                let change_request = DslChangeRequest {
                    case_id: ai_response.generated_cbu_id.clone(),
                    change_type: DslChangeType::Create,
                    dsl_content: ai_response.dsl_content.clone(),
                    metadata: HashMap::from([
                        ("ai_generated".to_string(), "true".to_string()),
                        ("scenario".to_string(), format!("{}", i + 1)),
                    ]),
                };

                match lifecycle_service.process_change(change_request).await {
                    Ok(lifecycle_result) => {
                        info!(
                            "✅ Lifecycle processing successful: {}",
                            lifecycle_result.success
                        );
                        info!(
                            "📊 Processing time: {}ms",
                            lifecycle_result.processing_time_ms
                        );
                    }
                    Err(e) => {
                        warn!("⚠️  Lifecycle processing failed: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("❌ AI onboarding failed: {}", e);
            }
        }

        let scenario_time = start_time.elapsed();
        info!(
            "⏱️  Scenario {} completed in {}ms\n",
            i + 1,
            scenario_time.as_millis()
        );
    }

    Ok(())
}

/// Run AI workflow without database (parsing and validation only)
async fn run_ai_workflow_no_db() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔄 Running AI workflow without database (parsing and validation)...");

    // Mock AI service for demonstration
    let test_requests = vec![
        (
            "UK Tech Startup",
            "Create onboarding case for TechCorp Ltd, UK corporation requiring custody services",
        ),
        (
            "German Fund",
            "Setup UCITS compliant fund Europa Capital GmbH in Germany",
        ),
        (
            "US Investment Manager",
            "Onboard Alpha Investments LLC for derivatives trading",
        ),
    ];

    let mut manager = CleanDslManager::new();
    let visualizer = DslVisualizer::new();

    for (name, instruction) in test_requests {
        info!("🧠 Processing: {}", name);

        // Generate mock DSL (simulating AI response)
        let mock_dsl = generate_mock_dsl_for_instruction(instruction);
        info!("📝 Mock DSL generated: {} characters", mock_dsl.len());

        // Parse and validate DSL
        match parse_dsl(&mock_dsl) {
            Ok(parsed_program) => {
                info!(
                    "✅ DSL parsing successful: {} statements",
                    parsed_program.len()
                );

                // Process through DSL manager
                match manager.process_dsl_request(mock_dsl.clone()).await {
                    result if result.success => {
                        info!("✅ DSL Manager processing successful");

                        // Generate visualization
                        let state_result = StateResult {
                            success: true,
                            case_id: format!("MOCK-{}", name.replace(" ", "-").to_uppercase()),
                            version_number: 1,
                            snapshot_id: "ai-demo-snapshot".to_string(),
                            errors: vec![],
                            processing_time_ms: 50,
                        };

                        let viz_result = visualizer.generate_visualization(&state_result).await;
                        if viz_result.success {
                            info!(
                                "🎨 Visualization generated: {} bytes",
                                viz_result.output_size_bytes
                            );
                        }
                    }
                    result => {
                        warn!("⚠️  DSL Manager processing had issues: {:?}", result.errors);
                    }
                }
            }
            Err(e) => {
                error!("❌ DSL parsing failed: {}", e);
            }
        }

        info!(""); // Empty line for readability
    }

    Ok(())
}

/// Run a mock AI demo when no API keys are available
async fn run_mock_ai_demo() -> Result<(), Box<dyn std::error::Error>> {
    info!("🎭 Running mock AI demo (no API keys required)...");

    let mock_scenarios = vec![
        ("TechCorp Ltd", "UK", "CORP", vec!["CUSTODY"]),
        (
            "Europa Capital GmbH",
            "DE",
            "FUND",
            vec!["CUSTODY", "ADMINISTRATION"],
        ),
        (
            "Alpha Investments LLC",
            "US",
            "INVESTMENT_MANAGER",
            vec!["PRIME_SERVICES"],
        ),
    ];

    for (client_name, jurisdiction, entity_type, services) in mock_scenarios {
        info!("🏢 Mock onboarding: {} ({})", client_name, jurisdiction);

        let mock_cbu_id = format!("CBU-{}-001", jurisdiction);
        let mock_dsl = format!(
            r#"(case.create
    :case-id "{}"
    :client-name "{}"
    :jurisdiction "{}"
    :entity-type "{}")
(services.provision :services {:?})
(kyc.start :case-id "{}")
(compliance.screen :case-id "{}")"#,
            mock_cbu_id, client_name, jurisdiction, entity_type, services, mock_cbu_id, mock_cbu_id
        );

        info!("🆔 Generated CBU: {}", mock_cbu_id);
        info!("📝 Mock DSL: {} characters", mock_dsl.len());

        // Validate the mock DSL
        match parse_dsl(&mock_dsl) {
            Ok(program) => {
                info!("✅ Mock DSL is valid: {} statements", program.len());
            }
            Err(e) => {
                warn!("⚠️  Mock DSL validation failed: {}", e);
            }
        }
    }

    info!("💡 To test with real AI, set OPENAI_API_KEY or GEMINI_API_KEY environment variable");
    Ok(())
}

/// Generate mock DSL based on instruction (simulating AI response)
fn generate_mock_dsl_for_instruction(instruction: &str) -> String {
    // Simple pattern matching to generate appropriate DSL
    let case_id = "DEMO-001";

    let base_dsl = format!(
        r#"(case.create :case-id "{}" :case-type "ONBOARDING")"#,
        case_id
    );

    let mut dsl_parts = vec![base_dsl];

    // Add service-specific DSL based on instruction
    if instruction.contains("custody") || instruction.contains("CUSTODY") {
        dsl_parts.push(format!(
            r#"(services.add :case-id "{}" :service-type "CUSTODY")"#,
            case_id
        ));
    }

    if instruction.contains("derivatives") || instruction.contains("DERIVATIVES") {
        dsl_parts.push(format!(
            r#"(services.add :case-id "{}" :service-type "DERIVATIVES")"#,
            case_id
        ));
        dsl_parts.push(format!(r#"(isda.establish_master :case-id "{}")"#, case_id));
    }

    if instruction.contains("UCITS") || instruction.contains("fund") {
        dsl_parts.push(format!(
            r#"(services.add :case-id "{}" :service-type "FUND_ADMINISTRATION")"#,
            case_id
        ));
        dsl_parts.push(format!(
            r#"(compliance.screen :case-id "{}" :regime "UCITS")"#,
            case_id
        ));
    }

    // Always add KYC
    dsl_parts.push(format!(r#"(kyc.start :case-id "{}")"#, case_id));

    dsl_parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_dsl_generation() {
        let instruction = "Create onboarding for UK tech company needing custody services";
        let dsl = generate_mock_dsl_for_instruction(instruction);

        assert!(dsl.contains("case.create"));
        assert!(dsl.contains("CUSTODY"));
        assert!(dsl.contains("kyc.start"));
    }

    #[test]
    fn test_derivatives_dsl_generation() {
        let instruction = "Onboard investment manager for derivatives trading";
        let dsl = generate_mock_dsl_for_instruction(instruction);

        assert!(dsl.contains("DERIVATIVES"));
        assert!(dsl.contains("isda.establish_master"));
    }

    #[tokio::test]
    async fn test_mock_demo_runs() {
        let result = run_mock_ai_demo().await;
        assert!(result.is_ok());
    }
}

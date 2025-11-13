//! Mock OpenAI Demo - Architecture Demo Without API Dependencies
//!
//! This example demonstrates the AI DSL architecture and workflow patterns
//! without requiring actual API keys or network calls. It showcases the
//! complete system architecture using mock implementations.
//!
//! ## What This Demonstrates:
//! 1. AI service architecture patterns
//! 2. DSL generation workflow simulation
//! 3. Complete onboarding pipeline
//! 4. Error handling and validation
//! 5. System integration patterns
//!
//! ## No Prerequisites:
//! - No API keys required
//! - No network connectivity needed
//! - Pure architectural demonstration
//!
//! ## Usage:
//! ```bash
//! cargo run --example mock_openai_demo
//! ```

use ob_poc::{
    dsl_manager::CleanDslManager,
    dsl_visualizer::{DslVisualizer, StateResult},
    parse_dsl,
};

use std::collections::HashMap;
use std::time::Instant;
use tokio;
use tracing::{info, warn};

/// Mock AI response structure
#[derive(Debug, Clone)]
struct MockAiResponse {
    pub generated_cbu_id: String,
    pub dsl_content: String,
    pub confidence_score: f64,
    pub processing_time_ms: u64,
    pub model_used: String,
    pub tokens_used: u32,
}

/// Mock AI service for demonstration
struct MockAiService {
    model_name: String,
    response_delay_ms: u64,
}

impl MockAiService {
    fn new(model_name: String) -> Self {
        Self {
            model_name,
            response_delay_ms: 150, // Simulate API response time
        }
    }

    async fn generate_onboarding_dsl(&self, request: &MockOnboardingRequest) -> MockAiResponse {
        // Simulate API call delay
        tokio::time::sleep(tokio::time::Duration::from_millis(self.response_delay_ms)).await;

        let cbu_id = self.generate_cbu_id(&request.jurisdiction, &request.entity_type);
        let dsl_content = self.generate_dsl_content(request, &cbu_id);

        MockAiResponse {
            generated_cbu_id: cbu_id,
            dsl_content,
            confidence_score: 0.92,
            processing_time_ms: self.response_delay_ms + 50,
            model_used: self.model_name.clone(),
            tokens_used: 450,
        }
    }

    fn generate_cbu_id(&self, jurisdiction: &str, entity_type: &str) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        format!(
            "CBU-{}-{}-{:03}",
            jurisdiction,
            entity_type,
            timestamp % 1000
        )
    }

    fn generate_dsl_content(&self, request: &MockOnboardingRequest, cbu_id: &str) -> String {
        let mut dsl_parts = Vec::new();

        // Base case creation
        dsl_parts.push(format!(
            r#"(case.create
    :case-id "{}"
    :client-name "{}"
    :jurisdiction "{}"
    :entity-type "{}"
    :case-type "ONBOARDING")"#,
            cbu_id, request.client_name, request.jurisdiction, request.entity_type
        ));

        // Add services based on request
        for service in &request.services {
            dsl_parts.push(format!(
                r#"(services.add :case-id "{}" :service-type "{}")"#,
                cbu_id, service
            ));
        }

        // Add KYC workflow
        dsl_parts.push(format!(r#"(kyc.start :case-id "{}")"#, cbu_id));

        // Add identity verification based on entity type
        let doc_type = match request.entity_type.as_str() {
            "CORP" => "INCORPORATION_CERTIFICATE",
            "FUND" => "FUND_DOCUMENTS",
            "INVESTMENT_MANAGER" => "REGISTRATION_CERTIFICATE",
            _ => "IDENTITY_DOCUMENT",
        };

        dsl_parts.push(format!(
            r#"(identity.verify
    :case-id "{}"
    :document-type "{}")"#,
            cbu_id, doc_type
        ));

        // Add compliance screening
        dsl_parts.push(format!(
            r#"(compliance.screen
    :case-id "{}"
    :screen-type "AML")"#,
            cbu_id
        ));

        // Add jurisdiction-specific requirements
        match request.jurisdiction.as_str() {
            "DE" => {
                dsl_parts.push(format!(
                    r#"(compliance.screen
    :case-id "{}"
    :screen-type "BAFIN")"#,
                    cbu_id
                ));
            }
            "US" => {
                dsl_parts.push(format!(
                    r#"(compliance.screen
    :case-id "{}"
    :screen-type "FINRA")"#,
                    cbu_id
                ));
            }
            "GB" => {
                dsl_parts.push(format!(
                    r#"(compliance.screen
    :case-id "{}"
    :screen-type "FCA")"#,
                    cbu_id
                ));
            }
            _ => {}
        }

        // Add service-specific workflows
        if request.services.contains(&"DERIVATIVES".to_string()) {
            dsl_parts.push(format!(r#"(isda.establish_master :case-id "{}")"#, cbu_id));
        }

        if request.services.contains(&"CUSTODY".to_string()) {
            dsl_parts.push(format!(
                r#"(services.configure
    :case-id "{}"
    :service "CUSTODY"
    :config-type "STANDARD")"#,
                cbu_id
            ));
        }

        // Final approval step
        dsl_parts.push(format!(r#"(case.approve :case-id "{}")"#, cbu_id));

        dsl_parts.join("\n\n")
    }
}

/// Mock onboarding request structure
#[derive(Debug, Clone)]
struct MockOnboardingRequest {
    pub instruction: String,
    pub client_name: String,
    pub jurisdiction: String,
    pub entity_type: String,
    pub services: Vec<String>,
    pub additional_context: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Mock OpenAI Demo Starting");
    info!("🎭 Architecture demo with mock AI implementation");

    let demo_start = Instant::now();

    // Step 1: Test Multiple AI Models
    test_multiple_mock_models().await?;

    // Step 2: Test Complex Scenarios
    test_complex_scenarios().await?;

    // Step 3: Test Error Handling
    test_error_scenarios().await?;

    // Step 4: Test Full Pipeline Integration
    test_full_pipeline_integration().await?;

    let total_time = demo_start.elapsed();
    info!(
        "🏁 Mock OpenAI Demo completed in {}ms",
        total_time.as_millis()
    );

    Ok(())
}

async fn test_multiple_mock_models() -> Result<(), Box<dyn std::error::Error>> {
    info!("🤖 Testing multiple mock AI models...");

    let models = vec![
        ("GPT-4", MockAiService::new("gpt-4".to_string())),
        (
            "GPT-3.5-Turbo",
            MockAiService::new("gpt-3.5-turbo".to_string()),
        ),
        ("Gemini-Pro", MockAiService::new("gemini-pro".to_string())),
    ];

    let test_request = MockOnboardingRequest {
        instruction: "Create comprehensive onboarding for hedge fund manager".to_string(),
        client_name: "Alpha Capital Management".to_string(),
        jurisdiction: "US".to_string(),
        entity_type: "INVESTMENT_MANAGER".to_string(),
        services: vec!["PRIME_SERVICES".to_string(), "DERIVATIVES".to_string()],
        additional_context: Some("Focus on equity derivatives trading".to_string()),
    };

    for (model_name, ai_service) in models {
        info!("🧠 Testing model: {}", model_name);

        let start_time = Instant::now();
        let response = ai_service.generate_onboarding_dsl(&test_request).await;
        let response_time = start_time.elapsed();

        info!(
            "  ✅ Response: CBU {}, {} chars, {:.2} confidence",
            response.generated_cbu_id,
            response.dsl_content.len(),
            response.confidence_score
        );
        info!(
            "  ⏱️  Timing: {}ms response, {} tokens",
            response_time.as_millis(),
            response.tokens_used
        );

        // Validate generated DSL
        match parse_dsl(&response.dsl_content) {
            Ok(program) => {
                info!("  🔍 DSL valid: {} statements", program.len());
            }
            Err(e) => {
                warn!("  ⚠️  DSL validation failed: {}", e);
            }
        }
        info!("");
    }

    Ok(())
}

async fn test_complex_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    info!("🏗️  Testing complex onboarding scenarios...");

    let scenarios = vec![
        MockOnboardingRequest {
            instruction: "Setup multi-jurisdiction UCITS fund with cross-border distribution"
                .to_string(),
            client_name: "Global Opportunities SICAV".to_string(),
            jurisdiction: "LU".to_string(),
            entity_type: "FUND".to_string(),
            services: vec![
                "CUSTODY".to_string(),
                "ADMINISTRATION".to_string(),
                "DISTRIBUTION".to_string(),
            ],
            additional_context: Some("UCITS compliant with EU passporting".to_string()),
        },
        MockOnboardingRequest {
            instruction: "Onboard crypto hedge fund with digital asset custody".to_string(),
            client_name: "Digital Alpha Fund LP".to_string(),
            jurisdiction: "US".to_string(),
            entity_type: "FUND".to_string(),
            services: vec!["DIGITAL_CUSTODY".to_string(), "PRIME_SERVICES".to_string()],
            additional_context: Some("Focus on cryptocurrency investments".to_string()),
        },
        MockOnboardingRequest {
            instruction: "Setup family office with comprehensive wealth management services"
                .to_string(),
            client_name: "Heritage Family Office".to_string(),
            jurisdiction: "CH".to_string(),
            entity_type: "FAMILY_OFFICE".to_string(),
            services: vec![
                "CUSTODY".to_string(),
                "PORTFOLIO_MANAGEMENT".to_string(),
                "REPORTING".to_string(),
            ],
            additional_context: Some("Ultra-high net worth family".to_string()),
        },
    ];

    let ai_service = MockAiService::new("gpt-4-complex".to_string());

    for (i, scenario) in scenarios.iter().enumerate() {
        info!("📋 Scenario {}: {}", i + 1, scenario.client_name);

        let response = ai_service.generate_onboarding_dsl(scenario).await;

        info!(
            "  🆔 CBU: {}, DSL: {} chars",
            response.generated_cbu_id,
            response.dsl_content.len()
        );

        // Test DSL processing
        let mut manager = CleanDslManager::new();
        let result = manager
            .process_dsl_request(response.dsl_content.clone())
            .await;

        if result.success {
            info!("  ✅ DSL processing successful");
        } else {
            warn!("  ⚠️  DSL processing issues: {:?}", result.errors);
        }

        // Test visualization
        let visualizer = DslVisualizer::new();
        let state_result = StateResult {
            success: true,
            case_id: response.generated_cbu_id.clone(),
            version_number: 1,
            snapshot_id: format!("scenario-{}", i + 1),
            errors: vec![],
            processing_time_ms: 100,
        };

        let viz_result = visualizer.generate_visualization(&state_result).await;
        if viz_result.success {
            info!("  🎨 Visualization: {} bytes", viz_result.output_size_bytes);
        }

        info!("");
    }

    Ok(())
}

async fn test_error_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    info!("⚠️  Testing error handling scenarios...");

    let error_scenarios = vec![
        (
            "Invalid Jurisdiction",
            MockOnboardingRequest {
                instruction: "Test invalid jurisdiction".to_string(),
                client_name: "Test Client".to_string(),
                jurisdiction: "INVALID".to_string(),
                entity_type: "CORP".to_string(),
                services: vec!["CUSTODY".to_string()],
                additional_context: None,
            },
        ),
        (
            "Empty Services",
            MockOnboardingRequest {
                instruction: "Test with no services".to_string(),
                client_name: "Empty Services Client".to_string(),
                jurisdiction: "US".to_string(),
                entity_type: "CORP".to_string(),
                services: vec![],
                additional_context: None,
            },
        ),
        (
            "Conflicting Requirements",
            MockOnboardingRequest {
                instruction: "Create individual account for corporation".to_string(),
                client_name: "Conflict Test Corp".to_string(),
                jurisdiction: "GB".to_string(),
                entity_type: "INDIVIDUAL".to_string(), // Conflict with corp name
                services: vec!["CUSTODY".to_string()],
                additional_context: None,
            },
        ),
    ];

    let ai_service = MockAiService::new("gpt-4-error-test".to_string());

    for (scenario_name, request) in error_scenarios {
        info!("🧪 Testing: {}", scenario_name);

        let response = ai_service.generate_onboarding_dsl(&request).await;

        // Even with problematic inputs, the mock should generate something
        info!("  📝 Generated DSL: {} chars", response.dsl_content.len());

        // Test if DSL is still parseable
        match parse_dsl(&response.dsl_content) {
            Ok(program) => {
                info!("  ✅ DSL parseable: {} statements", program.len());
            }
            Err(e) => {
                info!("  🔍 DSL parsing issue (expected): {}", e);
            }
        }

        info!("");
    }

    Ok(())
}

async fn test_full_pipeline_integration() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔗 Testing full pipeline integration...");

    let integration_request = MockOnboardingRequest {
        instruction: "Complete end-to-end onboarding for institutional client".to_string(),
        client_name: "Institutional Partners LLC".to_string(),
        jurisdiction: "US".to_string(),
        entity_type: "INVESTMENT_MANAGER".to_string(),
        services: vec![
            "CUSTODY".to_string(),
            "PRIME_SERVICES".to_string(),
            "DERIVATIVES".to_string(),
            "REPORTING".to_string(),
        ],
        additional_context: Some(
            "Large institutional client with complex requirements".to_string(),
        ),
    };

    let pipeline_start = Instant::now();

    // Step 1: AI Generation
    info!("  1️⃣  AI DSL Generation...");
    let ai_service = MockAiService::new("gpt-4-pipeline".to_string());
    let ai_response = ai_service
        .generate_onboarding_dsl(&integration_request)
        .await;
    info!("     ✅ Generated: {}", ai_response.generated_cbu_id);

    // Step 2: DSL Validation
    info!("  2️⃣  DSL Validation...");
    let parsed_program = parse_dsl(&ai_response.dsl_content)?;
    info!("     ✅ Validated: {} statements", parsed_program.len());

    // Step 3: DSL Manager Processing
    info!("  3️⃣  DSL Manager Processing...");
    let mut manager = CleanDslManager::new();
    let manager_result = manager
        .process_dsl_request(ai_response.dsl_content.clone())
        .await;
    if manager_result.success {
        info!("     ✅ Processing successful");
    } else {
        warn!("     ⚠️  Processing issues: {:?}", manager_result.errors);
    }

    // Step 4: Visualization Generation
    info!("  4️⃣  Visualization Generation...");
    let visualizer = DslVisualizer::new();
    let state_result = StateResult {
        success: true,
        case_id: ai_response.generated_cbu_id.clone(),
        version_number: 1,
        snapshot_id: "pipeline-integration".to_string(),
        errors: vec![],
        processing_time_ms: pipeline_start.elapsed().as_millis() as u64,
    };

    let viz_result = visualizer.generate_visualization(&state_result).await;
    if viz_result.success {
        info!("     ✅ Generated: {} bytes", viz_result.output_size_bytes);
    }

    // Step 5: Integration Summary
    let total_pipeline_time = pipeline_start.elapsed();
    info!(
        "  🏁 Pipeline completed in {}ms",
        total_pipeline_time.as_millis()
    );

    // Display final DSL for inspection
    info!("📄 Final Generated DSL:");
    println!("\n--- Generated DSL ---");
    println!("{}", ai_response.dsl_content);
    println!("--- End DSL ---\n");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_ai_service() {
        let service = MockAiService::new("test-model".to_string());
        let request = MockOnboardingRequest {
            instruction: "Test request".to_string(),
            client_name: "Test Client".to_string(),
            jurisdiction: "US".to_string(),
            entity_type: "CORP".to_string(),
            services: vec!["CUSTODY".to_string()],
            additional_context: None,
        };

        let response = service.generate_onboarding_dsl(&request).await;
        assert!(response.generated_cbu_id.starts_with("CBU-US-CORP"));
        assert!(response.dsl_content.contains("case.create"));
        assert!(response.confidence_score > 0.0);
    }

    #[test]
    fn test_cbu_id_generation() {
        let service = MockAiService::new("test".to_string());
        let cbu_id = service.generate_cbu_id("GB", "FUND");
        assert!(cbu_id.starts_with("CBU-GB-FUND"));
    }

    #[tokio::test]
    async fn test_full_demo_runs() {
        // This tests that the main demo function doesn't panic
        let result = test_multiple_mock_models().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_dsl_generation() {
        let service = MockAiService::new("test".to_string());
        let request = MockOnboardingRequest {
            instruction: "Simple test".to_string(),
            client_name: "Test Corp".to_string(),
            jurisdiction: "US".to_string(),
            entity_type: "CORP".to_string(),
            services: vec!["CUSTODY".to_string()],
            additional_context: None,
        };

        let dsl = service.generate_dsl_content(&request, "TEST-CBU");
        assert!(dsl.contains("case.create"));
        assert!(dsl.contains("TEST-CBU"));
        assert!(dsl.contains("CUSTODY"));
    }
}

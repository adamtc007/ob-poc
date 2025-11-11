//! AI DSL Onboarding Demo
//!
//! This example demonstrates the complete AI-powered DSL onboarding workflow:
//! 1. Generate CBU IDs for test clients
//! 2. Use AI to generate onboarding DSL from natural language
//! 3. Validate the generated DSL
//! 4. Simulate DSL execution through the system
//!
//! Usage:
//! export OPENAI_API_KEY="your-api-key"
//! cargo run --example ai_dsl_onboarding_demo

use chrono::Utc;
use ob_poc::ai::{AiConfig, AiDslRequest, AiDslResponse, AiResponseType, AiResult, AiService};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use uuid::Uuid;

// CBU Generator - copied locally to avoid database dependency
struct CbuGenerator;

impl CbuGenerator {
    fn generate_cbu_id(client_name: &str, jurisdiction: &str, entity_type: &str) -> String {
        let sanitized_name = client_name
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_uppercase();

        let short_name = if sanitized_name.len() > 8 {
            &sanitized_name[..8]
        } else {
            &sanitized_name
        };

        let timestamp = Utc::now().format("%m%d").to_string();
        let random_suffix: u16 = (Utc::now().timestamp_subsec_millis() % 1000) as u16;

        format!(
            "CBU-{}-{}-{}-{:03}",
            short_name,
            jurisdiction.to_uppercase(),
            entity_type.to_uppercase(),
            random_suffix
        )
    }

    fn generate_test_cbu_ids(count: usize) -> Vec<String> {
        let test_clients = vec![
            ("TechCorp Ltd", "GB", "CORP"),
            ("Alpha Capital Partners", "KY", "FUND"),
            ("Global Investments SA", "LU", "FUND"),
            ("Singapore Holdings Pte", "SG", "CORP"),
            ("Zenith Financial Group", "US", "CORP"),
        ];

        (0..count)
            .map(|i| {
                let (name, jurisdiction, entity_type) = &test_clients[i % test_clients.len()];
                Self::generate_cbu_id(name, jurisdiction, entity_type)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiOnboardingRequest {
    pub instruction: String,
    pub client_name: String,
    pub jurisdiction: String,
    pub entity_type: String,
    pub services: Vec<String>,
    pub compliance_level: Option<String>,
    pub context: HashMap<String, String>,
    pub ai_provider: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 AI DSL Onboarding Demo (Simplified)");
    println!("{}", "=".repeat(60));
    println!("Demonstrates AI-powered DSL generation workflow");

    // Check for API key
    let api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        println!("⚠️  OPENAI_API_KEY not set - running in simulation mode");
        "simulation-key".to_string()
    });

    let use_real_ai = !api_key.starts_with("simulation");

    if use_real_ai {
        println!("✅ OpenAI API key found - using real AI integration");
    } else {
        println!("🎭 Running in simulation mode - no API calls will be made");
    }

    // Demo 1: CBU ID Generation
    println!("\n📋 Demo 1: CBU ID Generation");
    println!("{}", "-".repeat(40));

    let test_cbu_ids = CbuGenerator::generate_test_cbu_ids(5);
    println!("Generated {} test CBU IDs:", test_cbu_ids.len());
    for (i, cbu_id) in test_cbu_ids.iter().enumerate() {
        println!("   {}. {}", i + 1, cbu_id);
    }

    // Demo 2: AI DSL Generation
    println!("\n🤖 Demo 2: AI DSL Generation");
    println!("{}", "-".repeat(40));

    if use_real_ai {
        let config = AiConfig::openai();
        let client = ob_poc::ai::openai::OpenAiClient::new(config)?;

        // Test health check
        match client.health_check().await {
            Ok(true) => println!("✅ OpenAI API health check: PASSED"),
            Ok(false) => println!("❌ OpenAI API health check: FAILED"),
            Err(e) => println!("❌ OpenAI API error: {}", e),
        }

        // Generate DSL for a test scenario
        let test_request = AiDslRequest {
            instruction: "Create complete onboarding DSL for TechCorp Ltd, a UK technology company needing custody services".to_string(),
            current_dsl: None,
            context: {
                let mut ctx = HashMap::new();
                ctx.insert("cbu_id".to_string(), test_cbu_ids[0].clone());
                ctx.insert("client_name".to_string(), "TechCorp Ltd".to_string());
                ctx.insert("jurisdiction".to_string(), "GB".to_string());
                ctx.insert("entity_type".to_string(), "CORP".to_string());
                ctx.insert("services".to_string(), "CUSTODY".to_string());
                ctx
            },
            response_type: AiResponseType::GenerateDsl,
            constraints: vec![
                "Use approved DSL v3.1 verbs only".to_string(),
                "Include case.create with CBU ID".to_string(),
                "Add products.add for custody".to_string(),
                "Include proper KYC workflow".to_string(),
            ],
        };

        match client.request_dsl(test_request).await {
            Ok(response) => {
                println!("✅ AI DSL Generation successful!");
                println!("\n📄 Generated DSL:");
                println!("{}", "-".repeat(30));
                println!("{}", response.dsl_content);
                println!("{}", "-".repeat(30));
                println!("\n💭 AI Explanation:");
                println!("{}", response.explanation);
                println!("\n🎯 Confidence: {:.1}%", response.confidence * 100.0);

                if !response.suggestions.is_empty() {
                    println!("\n💡 AI Suggestions:");
                    for suggestion in &response.suggestions {
                        println!("   • {}", suggestion);
                    }
                }

                // Demo 3: DSL Validation
                println!("\n🔍 Demo 3: DSL Syntax Validation");
                println!("{}", "-".repeat(40));

                match ob_poc::parser::parse_program(&response.dsl_content) {
                    Ok(forms) => {
                        println!("✅ DSL syntax validation: PASSED");
                        println!("   Parsed {} forms successfully", forms.len());

                        for (i, form) in forms.iter().take(3).enumerate() {
                            match form {
                                ob_poc::Form::Verb(verb_form) => {
                                    println!("   {}. Verb: {}", i + 1, verb_form.verb);
                                }
                                ob_poc::Form::Comment(_) => {
                                    println!("   {}. Comment", i + 1);
                                }
                            }
                        }
                        if forms.len() > 3 {
                            println!("   ... and {} more forms", forms.len() - 3);
                        }
                    }
                    Err(e) => {
                        println!("❌ DSL syntax validation: FAILED");
                        println!("   Error: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ AI DSL generation failed: {}", e);
            }
        }
    } else {
        println!("🎭 Simulating AI DSL Generation...");

        let mock_dsl = format!(
            r#"(case.create
  :cbu-id "{}"
  :nature-purpose "Technology services company"
  :jurisdiction "GB"
  :entity-name "TechCorp Ltd")

(products.add "CUSTODY")

(kyc.start
  :customer-id "{}"
  :method "standard_due_diligence"
  :jurisdictions ["GB"]
  :required-documents ["CertificateOfIncorporation" "ArticlesOfAssociation"])

(document.catalog
  :document-id "doc-cert-001"
  :document-type "CertificateOfIncorporation"
  :required true)"#,
            test_cbu_ids[0], test_cbu_ids[0]
        );

        println!("✅ Mock DSL Generation completed!");
        println!("\n📄 Generated DSL:");
        println!("{}", "-".repeat(30));
        println!("{}", mock_dsl);
        println!("{}", "-".repeat(30));
        println!("\n💭 Mock AI Explanation:");
        println!("Generated comprehensive onboarding DSL for TechCorp Ltd including case creation, custody product setup, and KYC initiation with UK corporate requirements.");
        println!("\n🎯 Mock Confidence: 92.0%");

        // Demo 3: DSL Validation
        println!("\n🔍 Demo 3: DSL Syntax Validation");
        println!("{}", "-".repeat(40));

        match ob_poc::parser::parse_program(&mock_dsl) {
            Ok(forms) => {
                println!("✅ DSL syntax validation: PASSED");
                println!("   Parsed {} forms successfully", forms.len());

                for (i, form) in forms.iter().enumerate() {
                    match form {
                        ob_poc::Form::Verb(verb_form) => {
                            println!("   {}. Verb: {}", i + 1, verb_form.verb);
                        }
                        ob_poc::Form::Comment(_) => {
                            println!("   {}. Comment", i + 1);
                        }
                    }
                }
            }
            Err(e) => {
                println!("❌ DSL syntax validation: FAILED");
                println!("   Error: {:?}", e);
            }
        }
    }

    // Real-world scenarios demonstration
    let onboarding_scenarios = vec![
        AiOnboardingRequest {
            instruction: "Create comprehensive onboarding for a UK technology company that needs custody services and fund accounting. They require enhanced due diligence due to complex ownership structure.".to_string(),
            client_name: "TechCorp Ltd".to_string(),
            jurisdiction: "GB".to_string(),
            entity_type: "CORP".to_string(),
            services: vec!["CUSTODY".to_string(), "FUND_ACCOUNTING".to_string()],
            compliance_level: Some("enhanced".to_string()),
            context: {
                let mut ctx = HashMap::new();
                ctx.insert("industry".to_string(), "technology".to_string());
                ctx.insert("ownership_complexity".to_string(), "high".to_string());
                ctx.insert("aum".to_string(), "50M_GBP".to_string());
                ctx
            },
            ai_provider: Some("openai".to_string()),
        },
        AiOnboardingRequest {
            instruction: "Set up onboarding for a Cayman Islands hedge fund focused on cryptocurrency investments. They need prime brokerage and derivatives trading capabilities.".to_string(),
            client_name: "Alpha Crypto Fund LP".to_string(),
            jurisdiction: "KY".to_string(),
            entity_type: "FUND".to_string(),
            services: vec!["PRIME_BROKERAGE".to_string(), "DERIVATIVES".to_string()],
            compliance_level: Some("standard".to_string()),
            context: {
                let mut ctx = HashMap::new();
                ctx.insert("strategy".to_string(), "cryptocurrency".to_string());
                ctx.insert("risk_profile".to_string(), "high".to_string());
                ctx.insert("target_aum".to_string(), "100M_USD".to_string());
                ctx
            },
            ai_provider: Some("openai".to_string()),
        },
        AiOnboardingRequest {
            instruction: "Create simple onboarding for a Luxembourg UCITS fund requiring basic custody and accounting services with standard compliance requirements.".to_string(),
            client_name: "European Equity UCITS".to_string(),
            jurisdiction: "LU".to_string(),
            entity_type: "UCITS".to_string(),
            services: vec!["CUSTODY".to_string(), "FUND_ACCOUNTING".to_string()],
            compliance_level: Some("standard".to_string()),
            context: {
                let mut ctx = HashMap::new();
                ctx.insert("strategy".to_string(), "european_equity".to_string());
                ctx.insert("ucits_compliance".to_string(), "required".to_string());
                ctx.insert("domicile".to_string(), "luxembourg".to_string());
                ctx
            },
            ai_provider: Some("openai".to_string()),
        },
    ];

    println!("\n📋 Demo 4: Business Scenario Overview");
    println!("{}", "-".repeat(40));
    println!("The following scenarios demonstrate different onboarding types:");
    for (i, scenario) in onboarding_scenarios.iter().enumerate() {
        println!(
            "   {}. {} ({}) - {} - Services: {}",
            i + 1,
            scenario.client_name,
            scenario.jurisdiction,
            scenario.entity_type,
            scenario.services.join(", ")
        );
    }

    // Pick first scenario for detailed demonstration
    let demo_scenario = &onboarding_scenarios[0];
    println!("\n🎯 Detailed Demo: {}", demo_scenario.client_name);
    println!("{}", "-".repeat(40));

    let generated_cbu = CbuGenerator::generate_cbu_id(
        &demo_scenario.client_name,
        &demo_scenario.jurisdiction,
        &demo_scenario.entity_type,
    );

    println!("📄 Generated CBU ID: {}", generated_cbu);
    println!("📋 Scenario: {}", demo_scenario.instruction);

    if use_real_ai {
        let config = AiConfig::openai();
        let client = ob_poc::ai::openai::OpenAiClient::new(config)?;

        let detailed_request = AiDslRequest {
            instruction: demo_scenario.instruction.clone(),
            current_dsl: None,
            context: {
                let mut ctx = demo_scenario.context.clone();
                ctx.insert("cbu_id".to_string(), generated_cbu.clone());
                ctx.insert("client_name".to_string(), demo_scenario.client_name.clone());
                ctx.insert(
                    "jurisdiction".to_string(),
                    demo_scenario.jurisdiction.clone(),
                );
                ctx.insert("entity_type".to_string(), demo_scenario.entity_type.clone());
                ctx.insert("services".to_string(), demo_scenario.services.join(", "));
                ctx
            },
            response_type: AiResponseType::GenerateDsl,
            constraints: vec![
                "Use approved DSL v3.1 verbs only".to_string(),
                "Include case.create with the provided CBU ID".to_string(),
                "Add products.add for requested services".to_string(),
                "Include comprehensive KYC workflow".to_string(),
                "Add document cataloging requirements".to_string(),
            ],
        };

        match client.request_dsl(detailed_request).await {
            Ok(response) => {
                println!("\n✅ Detailed AI DSL Generation Successful!");
                println!("\n📄 Enhanced DSL:");
                println!("{}", "-".repeat(30));
                println!("{}", response.dsl_content);
                println!("{}", "-".repeat(30));
                println!("\n💭 AI Analysis: {}", response.explanation);
                println!("\n🎯 Confidence: {:.1}%", response.confidence * 100.0);

                if !response.suggestions.is_empty() {
                    println!("\n💡 AI Enhancement Suggestions:");
                    for suggestion in &response.suggestions {
                        println!("   • {}", suggestion);
                    }
                }
            }
            Err(e) => {
                println!("❌ Enhanced DSL generation failed: {}", e);
            }
        }
    } else {
        println!("\n🎭 Would generate enhanced DSL with real AI integration");
        println!("   Set OPENAI_API_KEY to see full AI capabilities");
    }

    // Summary
    println!("\n🎉 AI DSL Onboarding Demo Complete!");
    println!("{}", "=".repeat(60));
    println!("📊 Summary of Capabilities Demonstrated:");
    println!("   ✅ CBU ID generation for unique client identification");
    println!("   ✅ AI-powered DSL generation from natural language");
    println!("   ✅ DSL syntax validation and parsing");
    println!("   ✅ Multiple business scenario support");
    println!("   ✅ Real-time AI integration capabilities");

    if use_real_ai {
        println!("\n💡 Production Features Demonstrated:");
        println!("   • Real-time OpenAI API integration");
        println!("   • Advanced DSL generation with business context");
        println!("   • Comprehensive validation and error handling");
        println!("   • Quality scoring and improvement suggestions");
    } else {
        println!("\n💡 To enable full AI features:");
        println!("   export OPENAI_API_KEY=\"your-api-key\"");
        println!("   cargo run --example ai_dsl_onboarding_demo");
    }

    println!("\n🚀 Ready for production AI-enhanced onboarding workflows!");
    println!("\n📋 Architecture Summary:");
    println!("┌─────────────┐    ┌─────────────┐    ┌─────────────┐");
    println!("│ Business    │───▶│ AI Service  │───▶│ DSL Parser  │");
    println!("│ Requirements│    │ (OpenAI)    │    │ & Validator │");
    println!("└─────────────┘    └─────────────┘    └─────────────┘");
    println!("       │                    │                    │");
    println!("   Natural Lang.      S-Expressions      Validated DSL");

    Ok(())
}

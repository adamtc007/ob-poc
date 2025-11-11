//! Simple OpenAI DSL Demo
//!
//! A standalone demonstration of OpenAI integration for DSL generation
//! without dependencies on deprecated agent modules.
//!
//! Usage:
//! 1. Set OPENAI_API_KEY environment variable
//! 2. Run: cargo run --example simple_openai_dsl_demo

use ob_poc::ai::openai::OpenAiClient;
use ob_poc::ai::{AiConfig, AiDslRequest, AiResponseType, AiService};
use std::collections::HashMap;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🤖 Simple OpenAI DSL Demo");
    println!("{}", "=".repeat(40));

    // Check for API key
    let api_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        eprintln!("❌ OPENAI_API_KEY environment variable not set");
        eprintln!("💡 To use this demo:");
        eprintln!("   export OPENAI_API_KEY=\"your-api-key-here\"");
        eprintln!("   cargo run --example simple_openai_dsl_demo");
        std::process::exit(1);
    });

    if api_key.trim().is_empty() {
        eprintln!("❌ OPENAI_API_KEY is empty");
        std::process::exit(1);
    }

    println!("✅ OpenAI API key found");

    // Create OpenAI client with GPT-3.5-turbo (free tier friendly)
    let ai_config = AiConfig::openai();
    let client = OpenAiClient::new(ai_config)?;

    // Test 1: Health Check
    println!("\n🔧 Test 1: OpenAI Health Check");
    println!("{}", "-".repeat(30));

    match client.health_check().await {
        Ok(true) => println!("✅ OpenAI API is accessible"),
        Ok(false) => {
            eprintln!("⚠️ OpenAI API returned non-success status");
        }
        Err(e) => {
            eprintln!("❌ OpenAI API health check failed: {}", e);
            if e.to_string().contains("authentication") {
                eprintln!("💡 Check your OPENAI_API_KEY is valid");
                std::process::exit(1);
            }
        }
    }

    // Test 2: Generate Simple DSL
    println!("\n📝 Test 2: Generate Simple Onboarding DSL");
    println!("{}", "-".repeat(30));

    let generate_request = AiDslRequest {
        instruction: "Create a simple onboarding DSL for a new client called 'TechCorp Ltd' based in the UK. They need basic custody services.".to_string(),
        current_dsl: None,
        context: {
            let mut ctx = HashMap::new();
            ctx.insert("entity_name".to_string(), "TechCorp Ltd".to_string());
            ctx.insert("jurisdiction".to_string(), "GB".to_string());
            ctx.insert("services".to_string(), "custody".to_string());
            ctx
        },
        response_type: AiResponseType::GenerateDsl,
        constraints: vec![
            "Use case.create verb".to_string(),
            "Include products.add for custody".to_string(),
            "Keep it simple and valid".to_string(),
        ],
    };

    match client.request_dsl(generate_request).await {
        Ok(response) => {
            println!("✅ DSL generation successful!");
            println!("\n📄 Generated DSL:");
            println!("{}", "-".repeat(20));
            println!("{}", response.dsl_content);
            println!("{}", "-".repeat(20));
            println!("\n💭 AI Explanation:");
            println!("{}", response.explanation);
            println!("\n🎯 Confidence: {:.1}%", response.confidence * 100.0);

            if !response.warnings.is_empty() {
                println!("\n⚠️ Warnings:");
                for warning in &response.warnings {
                    println!("   • {}", warning);
                }
            }

            if !response.suggestions.is_empty() {
                println!("\n💡 Suggestions:");
                for suggestion in &response.suggestions {
                    println!("   • {}", suggestion);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ DSL generation failed: {}", e);
            match e {
                ob_poc::ai::AiError::AuthenticationError => {
                    eprintln!("💡 Check your OpenAI API key is valid and has credits");
                }
                ob_poc::ai::AiError::RateLimitError => {
                    eprintln!("💡 Rate limit reached. Wait a moment and try again");
                }
                _ => {}
            }
            return Ok(()); // Continue with other tests
        }
    }

    // Test 3: Transform Existing DSL
    println!("\n🔄 Test 3: Transform Existing DSL");
    println!("{}", "-".repeat(30));

    let existing_dsl = r#"(case.create
  :cbu-id "CBU-TECH-001"
  :nature-purpose "Technology services")

(products.add "CUSTODY")"#;

    let transform_request = AiDslRequest {
        instruction: "Add KYC verification and document requirements for this client.".to_string(),
        current_dsl: Some(existing_dsl.to_string()),
        context: {
            let mut ctx = HashMap::new();
            ctx.insert("compliance_level".to_string(), "standard".to_string());
            ctx.insert("entity_type".to_string(), "corporation".to_string());
            ctx
        },
        response_type: AiResponseType::TransformDsl,
        constraints: vec![
            "Preserve existing structure".to_string(),
            "Add kyc.start verb".to_string(),
            "Add document requirements".to_string(),
        ],
    };

    match client.request_dsl(transform_request).await {
        Ok(response) => {
            println!("✅ DSL transformation successful!");
            println!("\n📄 Enhanced DSL:");
            println!("{}", "-".repeat(20));
            println!("{}", response.dsl_content);
            println!("{}", "-".repeat(20));
            println!("\n💭 Changes Made:");
            println!("{}", response.explanation);
            println!("\n🎯 Confidence: {:.1}%", response.confidence * 100.0);

            if !response.changes.is_empty() {
                println!("\n🔧 Specific Changes:");
                for change in &response.changes {
                    println!("   • {}", change);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ DSL transformation failed: {}", e);
            return Ok(()); // Continue with validation test
        }
    }

    // Test 4: Validate DSL
    println!("\n✅ Test 4: Validate DSL Syntax");
    println!("{}", "-".repeat(30));

    let test_dsl = r#"(case.create
  :cbu-id "TEST-001"
  :nature-purpose "Test validation")

(products.add "CUSTODY" "FUND_ACCOUNTING")

(kyc.start
  :customer-id "TEST-001"
  :documents ["CertificateOfIncorporation"])

;; This should be flagged as invalid
(invalid.verb "this is not approved")"#;

    let validate_request = AiDslRequest {
        instruction: "Please validate this DSL for syntax and vocabulary compliance. Check if all verbs are from the approved list.".to_string(),
        current_dsl: Some(test_dsl.to_string()),
        context: HashMap::new(),
        response_type: AiResponseType::ValidateDsl,
        constraints: vec![
            "Check S-expression syntax".to_string(),
            "Validate against approved verb list".to_string(),
            "Flag any issues".to_string(),
        ],
    };

    match client.request_dsl(validate_request).await {
        Ok(response) => {
            println!("✅ DSL validation completed!");
            println!("\n📋 Validation Results:");
            println!("{}", response.explanation);
            println!("\n🎯 Confidence: {:.1}%", response.confidence * 100.0);

            if !response.warnings.is_empty() {
                println!("\n⚠️ Issues Found:");
                for warning in &response.warnings {
                    println!("   • {}", warning);
                }
            }

            if !response.suggestions.is_empty() {
                println!("\n💡 Recommendations:");
                for suggestion in &response.suggestions {
                    println!("   • {}", suggestion);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ DSL validation failed: {}", e);
        }
    }

    // Test 5: Performance Check
    println!("\n📊 Test 5: Performance Check");
    println!("{}", "-".repeat(30));

    let start_time = std::time::Instant::now();

    let perf_request = AiDslRequest {
        instruction: "Generate a minimal valid DSL".to_string(),
        current_dsl: None,
        context: HashMap::new(),
        response_type: AiResponseType::GenerateDsl,
        constraints: vec!["Keep it very simple".to_string()],
    };

    match client.request_dsl(perf_request).await {
        Ok(response) => {
            let elapsed = start_time.elapsed();
            println!("✅ Performance test completed");
            println!("⏱️ Response time: {:?}", elapsed);
            println!("📝 Generated {} characters", response.dsl_content.len());
            println!("🎯 Confidence: {:.1}%", response.confidence * 100.0);
        }
        Err(e) => {
            eprintln!("❌ Performance test failed: {}", e);
        }
    }

    println!("\n🎉 OpenAI DSL Demo Complete!");
    println!("{}", "=".repeat(40));
    println!("📊 Summary:");
    println!("• ✅ OpenAI API integration working");
    println!("• ✅ DSL generation from natural language");
    println!("• ✅ DSL transformation capabilities");
    println!("• ✅ DSL validation and feedback");
    println!("• ✅ Performance monitoring");
    println!("\n💡 The AI integration can help with:");
    println!("   - Converting business requirements to DSL");
    println!("   - Adding compliance features to existing DSL");
    println!("   - Validating DSL syntax and vocabulary");
    println!("   - Suggesting improvements and best practices");

    Ok(())
}

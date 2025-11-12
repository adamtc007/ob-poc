//! Phase 3 Agentic DSL Generation Demo
//!
//! This demo showcases the complete Phase 3 agentic DSL generation capabilities
//! with canonical KYC orchestration templates and real AI integration.
//!
//! Features demonstrated:
//! - Canonical KYC investigation workflow generation
//! - UBO analysis workflow generation
//! - Template-based AI prompting with canonical forms
//! - DSL validation and normalization
//! - End-to-end workflow from natural language to executable DSL

use ob_poc::ai::dsl_service::{AiDslService, KycCaseRequest, UboAnalysisRequest};
use ob_poc::parser::{parse_normalize_and_validate, parse_program};
use std::env;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("🚀 Phase 3 Agentic DSL Generation Demo");
    println!("=====================================");

    // Check for API keys
    let has_openai = env::var("OPENAI_API_KEY").is_ok();
    let has_gemini = env::var("GEMINI_API_KEY").is_ok();

    if !has_openai && !has_gemini {
        println!(
            "⚠️  No AI API keys found. Set OPENAI_API_KEY or GEMINI_API_KEY to run with real AI."
        );
        println!("   Running in template demonstration mode...\n");
        demonstrate_templates_only().await?;
        return Ok(());
    }

    // Demo 1: KYC Investigation Generation
    if has_openai {
        println!("\n🧠 Demo 1: OpenAI KYC Investigation Generation");
        println!("===============================================");
        demonstrate_kyc_generation_openai().await?;
    }

    if has_gemini {
        println!("\n🧠 Demo 2: Gemini UBO Analysis Generation");
        println!("=========================================");
        demonstrate_ubo_generation_gemini().await?;
    }

    // Demo 3: End-to-End Workflow Validation
    println!("\n🔍 Demo 3: End-to-End Workflow Validation");
    println!("==========================================");
    demonstrate_end_to_end_validation().await?;

    // Demo 4: Canonical Form Validation
    println!("\n✅ Demo 4: Canonical Form Validation");
    println!("====================================");
    demonstrate_canonical_validation().await?;

    println!("\n🎉 Phase 3 Demo Complete!");
    println!("All canonical DSL generation capabilities working correctly.");

    Ok(())
}

/// Demonstrate KYC investigation generation with OpenAI
async fn demonstrate_kyc_generation_openai() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating AI DSL service with OpenAI...");
    let service = AiDslService::new_with_openai(None).await?;

    let kyc_request = KycCaseRequest {
        client_name: "Zenith Capital Partners".to_string(),
        jurisdiction: "GB".to_string(),
        entity_type: "HEDGE_FUND".to_string(),
        analyst_id: "analyst-sarah-jones".to_string(),
        business_reference: Some("KYC-2025-ZCP-001".to_string()),
        entity_properties: None,
        ubo_threshold: Some(25.0),
    };

    println!("Generating canonical KYC investigation workflow...");
    let response = service.generate_canonical_kyc_case(kyc_request).await?;

    println!("✅ Generation successful!");
    println!("📋 Template used: {}", response.template_used);
    println!("🎯 Confidence: {:.1}%", response.confidence * 100.0);
    println!("🏢 Generated entities: {:?}", response.entity_ids);
    println!("📄 Generated documents: {:?}", response.document_ids);

    if !response.warnings.is_empty() {
        println!("⚠️  Warnings: {:?}", response.warnings);
    }

    // Validate the generated DSL
    println!("\n🔍 Validating generated DSL...");
    match service.validate_canonical_dsl(&response.generated_dsl) {
        Ok(warnings) => {
            println!("✅ DSL validation passed!");
            if !warnings.is_empty() {
                println!("⚠️  Warnings: {:?}", warnings);
            }
        }
        Err(errors) => {
            println!("❌ DSL validation failed: {:?}", errors);
        }
    }

    // Show excerpt of generated DSL
    println!("\n📝 Generated DSL excerpt (first 500 chars):");
    println!("```");
    let excerpt = if response.generated_dsl.len() > 500 {
        format!("{}...", &response.generated_dsl[..500])
    } else {
        response.generated_dsl.clone()
    };
    println!("{}", excerpt);
    println!("```");

    // Test parsing with normalization
    println!("\n🔧 Testing parse and normalize pipeline...");
    match parse_and_normalize_and_validate(&response.generated_dsl) {
        Ok(ast) => {
            println!("✅ Parse, normalize, and validate successful!");
            println!(
                "📊 AST contains {} top-level statements",
                ast.statements.len()
            );
        }
        Err(e) => {
            println!("❌ Parse/normalize/validate failed: {}", e);
        }
    }

    Ok(())
}

/// Demonstrate UBO analysis generation with Gemini
async fn demonstrate_ubo_generation_gemini() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating AI DSL service with Gemini...");
    let service = AiDslService::new_with_gemini(None).await?;

    let ubo_request = UboAnalysisRequest {
        target_entity_name: "Phoenix Investment Fund LP".to_string(),
        target_entity_type: "INVESTMENT_FUND".to_string(),
        jurisdiction: "KY".to_string(),
        ubo_threshold: 25.0,
        ownership_structure: None,
        analyst_id: "analyst-michael-brown".to_string(),
    };

    println!("Generating canonical UBO analysis workflow...");
    let response = service.generate_canonical_ubo_analysis(ubo_request).await?;

    println!("✅ Generation successful!");
    println!("📋 Template used: {}", response.template_used);
    println!("🎯 Confidence: {:.1}%", response.confidence * 100.0);
    println!("🏢 Generated entities: {:?}", response.entity_ids);
    println!("📄 Generated documents: {:?}", response.document_ids);

    // Validate canonical forms
    println!("\n🔍 Validating canonical forms...");
    match service.validate_canonical_dsl(&response.generated_dsl) {
        Ok(warnings) => {
            println!("✅ Canonical validation passed!");
            if !warnings.is_empty() {
                println!("⚠️  Minor warnings: {:?}", warnings);
            }
        }
        Err(errors) => {
            println!("❌ Canonical validation failed: {:?}", errors);
        }
    }

    // Show UBO-specific patterns
    println!("\n📊 UBO-specific patterns detected:");
    let dsl = &response.generated_dsl;
    if dsl.contains("ubo.calc") {
        println!("✅ UBO calculation present");
    }
    if dsl.contains("ubo.outcome") {
        println!("✅ UBO outcome present");
    }
    if dsl.contains(":relationship-props") {
        println!("✅ Canonical relationship structure used");
    }
    if dsl.contains("entity.link") {
        println!("✅ Canonical entity linking used");
    }

    Ok(())
}

/// Demonstrate end-to-end validation workflow
async fn demonstrate_end_to_end_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing complete workflow: Generation → Parsing → Normalization → Validation");

    // Create a sample canonical DSL for testing
    let canonical_dsl = r#"
(case.create
  :case-id "test-kyc-case-001"
  :case-type "KYC_CASE"
  :business-reference "KYC-2025-TEST-001"
  :assigned-to "test-analyst"
  :title "Test KYC Investigation")

(entity.register
  :entity-id "test-entity-primary"
  :entity-type "HEDGE_FUND"
  :props {:legal-name "Test Hedge Fund LP"
          :jurisdiction "GB"
          :entity-status "ACTIVE"})

(entity.register
  :entity-id "test-person-ubo"
  :entity-type "PROPER_PERSON"
  :props {:legal-name "John Test Smith"
          :nationality "GB"})

(entity.link
  :link-id "test-ownership-link"
  :from-entity "test-person-ubo"
  :to-entity "test-entity-primary"
  :relationship-type "OWNERSHIP"
  :relationship-props {:ownership-percentage 100.0
                       :verification-status "VERIFIED"
                       :description "Complete ownership via control prong"})

(workflow.transition
  :to-state "approved"
  :reason "Test case approved for demonstration")

(ubo.outcome
  :target "test-entity-primary"
  :threshold 25.0
  :jurisdiction "GB"
  :ubos [{:entity "test-person-ubo"
          :effective-percent 100.0
          :prongs {:ownership true :control true}
          :evidence ["test-doc-1"]
          :confidence-score 95.0}])
"#;

    println!("📝 Testing canonical DSL structure...");

    // Step 1: Parse
    println!("🔧 Step 1: Parsing DSL...");
    let ast = match parse_program(canonical_dsl) {
        Ok(ast) => {
            println!("✅ Parsing successful - {} statements", ast.len());
            ast
        }
        Err(e) => {
            println!("❌ Parsing failed: {}", e);
            return Err(e.into());
        }
    };

    // Step 2: Full pipeline (should be no-op for canonical)
    println!("🔧 Step 2: Parse + Normalize + Validate pipeline...");
    match parse_normalize_and_validate(canonical_dsl) {
        Ok((validated_ast, _validation_result)) => {
            println!("✅ Full pipeline successful!");
            println!("📊 Final AST has {} statements", validated_ast.len());

            // Check if normalization changed anything (should be minimal for canonical)
            if ast.len() == validated_ast.len() {
                println!(
                    "✅ No structural changes during normalization (as expected for canonical DSL)"
                );
            }
        }
        Err(e) => {
            println!("❌ Pipeline failed: {}", e);
            return Err(e.into());
        }
    }

    // Step 3: Verify canonical patterns
    println!("🔧 Step 3: Verifying canonical patterns...");
    let canonical_checks = vec![
        (canonical_dsl.contains("case.create"), "case.create verb"),
        (canonical_dsl.contains(":case-id"), "kebab-case case ID"),
        (canonical_dsl.contains("entity.link"), "entity.link verb"),
        (
            canonical_dsl.contains(":relationship-props"),
            "relationship props structure",
        ),
        (
            canonical_dsl.contains("workflow.transition"),
            "workflow.transition verb",
        ),
        (
            canonical_dsl.contains(":to-state"),
            "canonical to-state key",
        ),
        (
            canonical_dsl.contains("ubo.outcome"),
            "UBO outcome structure",
        ),
    ];

    for (check, description) in canonical_checks {
        if check {
            println!("✅ {}", description);
        } else {
            println!("❌ Missing: {}", description);
        }
    }

    Ok(())
}

/// Demonstrate canonical form validation
async fn demonstrate_canonical_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing canonical vs legacy DSL validation...");

    // Test 1: Legacy DSL (should trigger warnings/errors)
    println!("\n🧪 Test 1: Legacy DSL validation");
    let legacy_dsl = r#"
(kyc.start_case :case_id "legacy-case" :case_type "KYC")
(ubo.link_ownership :from "person1" :to "company1" :percentage 50.0)
(kyc.add_finding :case_id "legacy-case" :finding "Some finding")
"#;

    // We need a service for validation - use mock if no API keys
    let service_result = if env::var("OPENAI_API_KEY").is_ok() {
        AiDslService::new_with_openai(None).await
    } else if env::var("GEMINI_API_KEY").is_ok() {
        AiDslService::new_with_gemini(None).await
    } else {
        println!("⚠️  No API keys - skipping service-based validation");
        return Ok(());
    };

    let service = service_result?;

    match service.validate_canonical_dsl(legacy_dsl) {
        Ok(warnings) => {
            println!("⚠️  Legacy DSL passed with warnings: {:?}", warnings);
        }
        Err(errors) => {
            println!("❌ Legacy DSL failed validation (expected): {:?}", errors);
        }
    }

    // Test 2: Canonical DSL (should pass cleanly)
    println!("\n🧪 Test 2: Canonical DSL validation");
    let canonical_dsl = r#"
(case.create :case-id "canonical-case" :case-type "KYC_CASE")
(entity.link :from-entity "person1" :to-entity "company1"
             :relationship-props {:ownership-percentage 50.0})
(case.update :case-id "canonical-case" :notes "Finding added via canonical form")
"#;

    match service.validate_canonical_dsl(canonical_dsl) {
        Ok(warnings) => {
            println!("✅ Canonical DSL validation passed!");
            if !warnings.is_empty() {
                println!("📝 Minor warnings: {:?}", warnings);
            }
        }
        Err(errors) => {
            println!(
                "❌ Unexpected canonical DSL validation failure: {:?}",
                errors
            );
        }
    }

    // Test 3: Mixed DSL (canonical + legacy)
    println!("\n🧪 Test 3: Mixed DSL validation");
    let mixed_dsl = r#"
(case.create :case-id "mixed-case" :case-type "KYC_CASE")
(kyc.start_case :case_id "mixed-case")
(entity.link :relationship-props {:ownership-percentage 75.0})
(ubo.link_ownership :percentage 25.0)
"#;

    match service.validate_canonical_dsl(mixed_dsl) {
        Ok(warnings) => {
            println!("⚠️  Mixed DSL passed with warnings: {:?}", warnings);
        }
        Err(errors) => {
            println!("❌ Mixed DSL validation errors: {:?}", errors);
        }
    }

    Ok(())
}

/// Demonstrate templates without AI (fallback mode)
async fn demonstrate_templates_only() -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Template Structure Demonstration");
    println!("==================================");

    // Load and display template structures
    let kyc_template = include_str!("../src/ai/prompts/canonical/kyc_investigation.template");
    let ubo_template = include_str!("../src/ai/prompts/canonical/ubo_analysis.template");

    println!("🔍 KYC Investigation Template Structure:");
    let kyc_lines: Vec<&str> = kyc_template.lines().take(20).collect();
    for line in kyc_lines {
        if line.trim().starts_with("(") || line.trim().starts_with(";") {
            println!("  {}", line);
        }
    }
    println!(
        "  ... (truncated, {} total lines)",
        kyc_template.lines().count()
    );

    println!("\n🔍 UBO Analysis Template Structure:");
    let ubo_lines: Vec<&str> = ubo_template.lines().take(20).collect();
    for line in ubo_lines {
        if line.trim().starts_with("(") || line.trim().starts_with(";") {
            println!("  {}", line);
        }
    }
    println!(
        "  ... (truncated, {} total lines)",
        ubo_template.lines().count()
    );

    // Show canonical instructions summary
    let instructions = include_str!("../src/ai/prompts/canonical/canonical_instructions.md");
    println!("\n📖 Canonical Instructions Summary:");
    for line in instructions.lines().take(30) {
        if line.starts_with("##") || line.starts_with("- `") {
            println!("  {}", line);
        }
    }

    // Demonstrate template variable extraction
    println!("\n🔧 Template Variables (examples):");
    let variables = vec![
        "{case-id}",
        "{business-reference}",
        "{analyst-id}",
        "{legal-name}",
        "{jurisdiction}",
        "{entity-type}",
        "{ownership-percent}",
        "{ubo-threshold}",
    ];

    for var in variables {
        if kyc_template.contains(var) {
            println!("  ✅ {} found in KYC template", var);
        }
        if ubo_template.contains(var) {
            println!("  ✅ {} found in UBO template", var);
        }
    }

    println!("\n💡 To see full AI generation in action, set OPENAI_API_KEY or GEMINI_API_KEY");

    Ok(())
}

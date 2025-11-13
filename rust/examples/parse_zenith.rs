//! Parse Zenith Example - DSL Parsing Demo with UBO Case Study
//!
//! This example demonstrates DSL parsing capabilities using the Zenith Capital
//! UBO (Ultimate Beneficial Ownership) case study. It showcases complex DSL
//! structures, entity relationships, and ownership calculations.
//!
//! ## What This Demonstrates:
//! 1. Complex DSL parsing with nested structures
//! 2. UBO resolution workflows
//! 3. Entity relationship modeling
//! 4. Ownership threshold calculations
//! 5. Multi-domain DSL integration
//!
//! ## Usage:
//! ```bash
//! cargo run --example parse_zenith
//! ```

use ob_poc::{
    dsl_manager::CleanDslManager,
    dsl_visualizer::{DslVisualizer, StateResult},
    parse_dsl,
};

use std::collections::HashMap;
use std::time::Instant;
use tokio;
use tracing::{error, info, warn};

/// Zenith Capital UBO case study DSL
const ZENITH_CAPITAL_DSL: &str = r#"
;; Zenith Capital UBO Resolution Case Study
;; Complex ownership structure with multiple layers and jurisdictions

(case.create
    :case-id "CBU-UBO-ZENITH-001"
    :client-name "Zenith Capital Management Ltd"
    :jurisdiction "GB"
    :entity-type "INVESTMENT_MANAGER"
    :case-type "UBO_RESOLUTION")

;; Register primary entity
(entity.register
    :entity-id "ZENITH-MAIN"
    :entity-name "Zenith Capital Management Ltd"
    :jurisdiction "GB"
    :entity-type "LIMITED_COMPANY"
    :incorporation-date "2018-03-15")

;; Register holding company
(entity.register
    :entity-id "ZENITH-HOLDINGS"
    :entity-name "Zenith Holdings PLC"
    :jurisdiction "GB"
    :entity-type "PUBLIC_LIMITED_COMPANY"
    :incorporation-date "2015-01-20")

;; Register parent fund structure
(entity.register
    :entity-id "GLOBAL-FUND-LP"
    :entity-name "Global Investment Fund LP"
    :jurisdiction "DE"
    :entity-type "LIMITED_PARTNERSHIP"
    :incorporation-date "2012-08-10")

;; Register ultimate individuals
(entity.register
    :entity-id "INDIVIDUAL-001"
    :entity-name "Sarah Chen"
    :jurisdiction "GB"
    :entity-type "INDIVIDUAL"
    :birth-date "1975-06-12")

(entity.register
    :entity-id "INDIVIDUAL-002"
    :entity-name "Michael Rodriguez"
    :jurisdiction "US"
    :entity-type "INDIVIDUAL"
    :birth-date "1968-11-03")

(entity.register
    :entity-id "INDIVIDUAL-003"
    :entity-name "Elena Volkov"
    :jurisdiction "CH"
    :entity-type "INDIVIDUAL"
    :birth-date "1972-04-28")

;; Define ownership relationships
(ubo.collect-entity-data
    :case-id "CBU-UBO-ZENITH-001"
    :entity-id "ZENITH-MAIN"
    :ownership-threshold 25.0)

;; Direct ownership: Zenith Holdings -> Zenith Main (65%)
(edge
    :from "ZENITH-HOLDINGS"
    :to "ZENITH-MAIN"
    :relationship-type "SHAREHOLDING"
    :ownership-percentage 65.0
    :voting-rights 65.0
    :control-type "DIRECT")

;; Complex ownership: Global Fund -> Zenith Holdings (45%)
(edge
    :from "GLOBAL-FUND-LP"
    :to "ZENITH-HOLDINGS"
    :relationship-type "SHAREHOLDING"
    :ownership-percentage 45.0
    :voting-rights 40.0
    :control-type "DIRECT")

;; Individual ownership in Global Fund
(edge
    :from "INDIVIDUAL-001"
    :to "GLOBAL-FUND-LP"
    :relationship-type "LIMITED_PARTNER"
    :ownership-percentage 35.0
    :voting-rights 0.0
    :control-type "ECONOMIC")

(edge
    :from "INDIVIDUAL-002"
    :to "GLOBAL-FUND-LP"
    :relationship-type "GENERAL_PARTNER"
    :ownership-percentage 30.0
    :voting-rights 100.0
    :control-type "OPERATIONAL")

;; Direct individual ownership in Zenith Holdings
(edge
    :from "INDIVIDUAL-003"
    :to "ZENITH-HOLDINGS"
    :relationship-type "SHAREHOLDING"
    :ownership-percentage 25.0
    :voting-rights 25.0
    :control-type "DIRECT")

;; Remaining ownership (market/institutional)
(edge
    :from "MARKET-INVESTORS"
    :to "ZENITH-HOLDINGS"
    :relationship-type "SHAREHOLDING"
    :ownership-percentage 30.0
    :voting-rights 30.0
    :control-type "DISPERSED")

;; Get complete ownership structure
(ubo.get-ownership-structure
    :case-id "CBU-UBO-ZENITH-001"
    :entity-id "ZENITH-MAIN"
    :max-depth 5)

;; Calculate indirect ownership percentages
(ubo.calculate-indirect-ownership
    :case-id "CBU-UBO-ZENITH-001"
    :target-entity "ZENITH-MAIN"
    :threshold 25.0)

;; Resolve ultimate beneficial owners
(ubo.resolve-ubos
    :case-id "CBU-UBO-ZENITH-001"
    :entity-id "ZENITH-MAIN"
    :threshold 25.0
    :include-indirect true)

;; Identity verification for identified UBOs
(identity.verify
    :case-id "CBU-UBO-ZENITH-001"
    :entity-id "INDIVIDUAL-002"
    :document-type "PASSPORT"
    :jurisdiction "US")

(identity.verify
    :case-id "CBU-UBO-ZENITH-001"
    :entity-id "INDIVIDUAL-003"
    :document-type "NATIONAL_ID"
    :jurisdiction "CH")

;; Enhanced due diligence for complex structure
(kyc.assess
    :case-id "CBU-UBO-ZENITH-001"
    :assessment-type "ENHANCED"
    :reason "COMPLEX_OWNERSHIP_STRUCTURE")

;; Compliance screening for all jurisdictions
(compliance.screen
    :case-id "CBU-UBO-ZENITH-001"
    :screen-type "MULTI_JURISDICTION"
    :jurisdictions ["GB" "DE" "US" "CH"])

;; UBO calculation outcome
(ubo.outcome
    :case-id "CBU-UBO-ZENITH-001"
    :identified-ubos ["INDIVIDUAL-002" "INDIVIDUAL-003"]
    :indirect-ownership-calculated true
    :threshold-met true
    :risk-rating "MEDIUM")

;; Document the complete UBO registry entry
(document.catalog
    :case-id "CBU-UBO-ZENITH-001"
    :document-type "UBO_REGISTER"
    :entities ["ZENITH-MAIN" "ZENITH-HOLDINGS" "GLOBAL-FUND-LP"]
    :individuals ["INDIVIDUAL-001" "INDIVIDUAL-002" "INDIVIDUAL-003"])

;; Case approval after successful UBO resolution
(case.approve
    :case-id "CBU-UBO-ZENITH-001"
    :approval-type "UBO_COMPLETE"
    :approved-by "UBO_RESOLUTION_TEAM")
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Parse Zenith Demo Starting");
    info!("🏢 Zenith Capital UBO Resolution Case Study");

    let demo_start = Instant::now();

    // Step 1: Parse the Zenith Capital DSL
    test_zenith_parsing().await?;

    // Step 2: Process through DSL Manager
    test_dsl_manager_processing().await?;

    // Step 3: Generate visualization
    test_visualization_generation().await?;

    // Step 4: Analyze DSL structure
    analyze_dsl_structure().await?;

    let total_time = demo_start.elapsed();
    info!(
        "🏁 Parse Zenith Demo completed in {}ms",
        total_time.as_millis()
    );

    Ok(())
}

async fn test_zenith_parsing() -> Result<(), Box<dyn std::error::Error>> {
    info!("📖 Step 1: Parsing Zenith Capital DSL...");

    let parse_start = Instant::now();

    match parse_dsl(ZENITH_CAPITAL_DSL) {
        Ok(parsed_program) => {
            let parse_time = parse_start.elapsed();

            info!("✅ DSL parsing successful!");
            info!(
                "📊 Parsed {} forms in {}ms",
                parsed_program.len(),
                parse_time.as_millis()
            );

            // Analyze form types
            let mut form_counts = HashMap::new();
            for form in &parsed_program {
                if let ob_poc::parser_ast::Form::Verb(verb_form) = form {
                    let verb = verb_form.verb.clone();
                    *form_counts.entry(verb).or_insert(0) += 1;
                }
            }

            info!("📈 Form analysis:");
            for (verb, count) in form_counts.iter() {
                info!("   {} operations: {}", verb, count);
            }

            // Show sample forms
            info!("🔍 Sample parsed forms:");
            for (i, form) in parsed_program.iter().take(3).enumerate() {
                match form {
                    ob_poc::parser_ast::Form::Verb(verb_form) => {
                        info!(
                            "   {}: {} with {} attributes",
                            i + 1,
                            verb_form.verb,
                            verb_form.pairs.len()
                        );
                    }
                    ob_poc::parser_ast::Form::Comment(comment) => {
                        info!("   {}: Comment ({} chars)", i + 1, comment.len());
                    }
                }
            }
        }
        Err(e) => {
            error!("❌ DSL parsing failed: {}", e);
            return Err(Box::new(e));
        }
    }

    Ok(())
}

async fn test_dsl_manager_processing() -> Result<(), Box<dyn std::error::Error>> {
    info!("⚙️  Step 2: Processing through DSL Manager...");

    let mut manager = CleanDslManager::new();
    let process_start = Instant::now();

    let result = manager
        .process_dsl_request(ZENITH_CAPITAL_DSL.to_string())
        .await;

    let process_time = process_start.elapsed();

    if result.success {
        info!("✅ DSL Manager processing successful!");
        info!("⏱️  Processing completed in {}ms", process_time.as_millis());

        info!("📋 Processing completed successfully");
    } else {
        warn!("⚠️  DSL Manager processing had issues:");
        for error in &result.errors {
            warn!("   ❌ {}", error);
        }
    }

    Ok(())
}

async fn test_visualization_generation() -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Step 3: Generating visualization...");

    let visualizer = DslVisualizer::new();
    let viz_start = Instant::now();

    let state_result = StateResult {
        success: true,
        case_id: "CBU-UBO-ZENITH-001".to_string(),
        version_number: 1,
        snapshot_id: "zenith-parse-demo".to_string(),
        errors: vec![],
        processing_time_ms: 150,
    };

    let viz_result = visualizer.generate_visualization(&state_result).await;
    let viz_time = viz_start.elapsed();

    if viz_result.success {
        info!("✅ Visualization generation successful!");
        info!(
            "📊 Generated {} bytes in {}ms",
            viz_result.output_size_bytes,
            viz_time.as_millis()
        );
        info!("🎯 Chart type: {:?}", viz_result.chart_type);
        info!("📄 Output format: {:?}", viz_result.format);

        if !viz_result.warnings.is_empty() {
            info!("⚠️  Visualization warnings:");
            for warning in &viz_result.warnings {
                warn!("   {}", warning);
            }
        }
    } else {
        warn!("⚠️  Visualization generation had issues:");
        for error in &viz_result.errors {
            warn!("   ❌ {}", error);
        }
    }

    Ok(())
}

async fn analyze_dsl_structure() -> Result<(), Box<dyn std::error::Error>> {
    info!("🔬 Step 4: Analyzing DSL structure...");

    // Parse again for detailed analysis
    let parsed_program = parse_dsl(ZENITH_CAPITAL_DSL)?;

    // Analyze UBO-specific operations
    let ubo_operations: Vec<_> = parsed_program
        .iter()
        .filter_map(|form| match form {
            ob_poc::parser_ast::Form::Verb(verb_form) if verb_form.verb.starts_with("ubo.") => {
                Some(verb_form)
            }
            _ => None,
        })
        .collect();

    info!("🏢 UBO Operations Analysis:");
    info!("   Found {} UBO-specific operations", ubo_operations.len());

    for op in &ubo_operations {
        info!("   • {}", op.verb);
        if let Some(entity_id) = op.pairs.get(&ob_poc::parser_ast::Key::new("entity-id")) {
            info!("     Target entity: {:?}", entity_id);
        }
        if let Some(threshold) = op.pairs.get(&ob_poc::parser_ast::Key::new("threshold")) {
            info!("     Threshold: {:?}", threshold);
        }
    }

    // Analyze entity registrations
    let entity_registrations: Vec<_> = parsed_program
        .iter()
        .filter_map(|form| match form {
            ob_poc::parser_ast::Form::Verb(verb_form) if verb_form.verb == "entity.register" => {
                Some(verb_form)
            }
            _ => None,
        })
        .collect();

    info!("👥 Entity Registration Analysis:");
    info!(
        "   Found {} entity registrations",
        entity_registrations.len()
    );

    let mut entity_types = HashMap::new();
    let mut jurisdictions = HashMap::new();

    for reg in &entity_registrations {
        if let Some(entity_type) = reg.pairs.get(&ob_poc::parser_ast::Key::new("entity-type")) {
            *entity_types
                .entry(format!("{:?}", entity_type))
                .or_insert(0) += 1;
        }
        if let Some(jurisdiction) = reg.pairs.get(&ob_poc::parser_ast::Key::new("jurisdiction")) {
            *jurisdictions
                .entry(format!("{:?}", jurisdiction))
                .or_insert(0) += 1;
        }
    }

    info!("   Entity types:");
    for (entity_type, count) in entity_types {
        info!("     {} entities: {}", entity_type, count);
    }

    info!("   Jurisdictions:");
    for (jurisdiction, count) in jurisdictions {
        info!("     {}: {} entities", jurisdiction, count);
    }

    // Analyze ownership edges
    let ownership_edges: Vec<_> = parsed_program
        .iter()
        .filter_map(|form| match form {
            ob_poc::parser_ast::Form::Verb(verb_form) if verb_form.verb == "edge" => {
                Some(verb_form)
            }
            _ => None,
        })
        .collect();

    info!("🔗 Ownership Structure Analysis:");
    info!("   Found {} ownership relationships", ownership_edges.len());

    let mut relationship_types = HashMap::new();
    let mut total_ownership = 0.0;
    let mut ownership_count = 0;

    for edge in &ownership_edges {
        if let Some(rel_type) = edge
            .pairs
            .get(&ob_poc::parser_ast::Key::new("relationship-type"))
        {
            *relationship_types
                .entry(format!("{:?}", rel_type))
                .or_insert(0) += 1;
        }
        if let Some(ownership) = edge
            .pairs
            .get(&ob_poc::parser_ast::Key::new("ownership-percentage"))
        {
            if let ob_poc::parser_ast::Value::Double(percentage) = ownership {
                total_ownership += percentage;
                ownership_count += 1;
            }
        }
    }

    info!("   Relationship types:");
    for (rel_type, count) in relationship_types {
        info!("     {}: {}", rel_type, count);
    }

    if ownership_count > 0 {
        let avg_ownership = total_ownership / ownership_count as f64;
        info!("   Average ownership percentage: {:.2}%", avg_ownership);
    }

    // Calculate complexity score
    let complexity_score = calculate_complexity_score(&parsed_program);
    info!("📈 DSL Complexity Score: {:.2}", complexity_score);

    match complexity_score {
        score if score >= 8.0 => info!("   Assessment: Very Complex Structure"),
        score if score >= 6.0 => info!("   Assessment: Complex Structure"),
        score if score >= 4.0 => info!("   Assessment: Moderate Complexity"),
        _ => info!("   Assessment: Simple Structure"),
    }

    Ok(())
}

fn calculate_complexity_score(program: &ob_poc::parser_ast::Program) -> f64 {
    let mut score = 0.0;

    // Base score from form count
    score += program.len() as f64 * 0.1;

    // Additional points for specific operation types
    for form in program {
        match form {
            ob_poc::parser_ast::Form::Verb(verb_form) => {
                match verb_form.verb.as_str() {
                    verb if verb.starts_with("ubo.") => score += 1.0,
                    "edge" => score += 0.8,
                    "entity.register" => score += 0.5,
                    "identity.verify" => score += 0.6,
                    "compliance.screen" => score += 0.7,
                    _ => score += 0.2,
                }

                // Additional complexity for many attributes
                if verb_form.pairs.len() > 5 {
                    score += 0.3;
                }
            }
            ob_poc::parser_ast::Form::Comment(_) => score += 0.1,
        }
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zenith_dsl_is_valid() {
        let result = parse_dsl(ZENITH_CAPITAL_DSL);
        assert!(result.is_ok(), "Zenith Capital DSL should be valid");

        let program = result.unwrap();
        assert!(
            program.statements.len() > 10,
            "Should have multiple statements"
        );

        // Check for key operations
        let has_case_create = program.iter().any(|form| match form {
            ob_poc::parser_ast::Form::Verb(verb_form) => verb_form.verb == "case.create",
            _ => false,
        });
        let has_ubo_operations = program.iter().any(|form| match form {
            ob_poc::parser_ast::Form::Verb(verb_form) => verb_form.verb.starts_with("ubo."),
            _ => false,
        });
        let has_entity_register = program.iter().any(|form| match form {
            ob_poc::parser_ast::Form::Verb(verb_form) => verb_form.verb == "entity.register",
            _ => false,
        });

        assert!(has_case_create, "Should have case.create");
        assert!(has_ubo_operations, "Should have UBO operations");
        assert!(has_entity_register, "Should have entity registrations");
    }

    #[test]
    fn test_complexity_calculation() {
        let program = parse_dsl(ZENITH_CAPITAL_DSL).unwrap();
        let score = calculate_complexity_score(&program);

        assert!(score > 5.0, "Zenith case should be moderately complex");
        assert!(score < 15.0, "Score should be reasonable");
    }

    #[tokio::test]
    async fn test_zenith_parsing_functions() {
        let result = test_zenith_parsing().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_analysis_functions() {
        let result = analyze_dsl_structure().await;
        assert!(result.is_ok());
    }
}

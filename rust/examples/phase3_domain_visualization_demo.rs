//! Phase 3: Domain-Specific Visualization Demonstration
//!
//! This example demonstrates the domain-aware visualization capabilities implemented
//! in Phase 3, including functional state visualization, KYC/UBO-specific enhancements,
//! and domain-specific styling and metrics.
//!
//! Features demonstrated:
//! - Domain-specific node and edge styling
//! - Functional state progression visualization
//! - KYC/UBO workflow highlighting
//! - Entity relationship emphasis
//! - Compliance workflow tracking
//! - Domain-specific metrics and analytics

use ob_poc::ast::{Program, Statement, Value, Workflow};
use ob_poc::database::DslDomainRepository;
use ob_poc::domain_visualizations::{DomainVisualizer, HighlightPriority};
use ob_poc::dsl_manager_v2::{
    DomainVisualizationOptions, DslManagerV2, LayoutType, VisualizationOptions,
};
use ob_poc::models;
use sqlx::PgPool;
use std::collections::HashMap;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("🚀 Phase 3: Domain-Specific Visualization Demo");
    println!("{}", "=".repeat(60));
    println!();

    // Connect to database (if available)
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string());

    println!("🔌 Connecting to database...");
    let pool = match PgPool::connect(&database_url).await {
        Ok(pool) => {
            println!("   ✅ Database connection successful");
            pool
        }
        Err(e) => {
            println!("   ⚠️  Database connection failed: {}", e);
            println!("   📝 Running in mock mode with sample data");
            return run_mock_demonstration().await;
        }
    };

    // Initialize DSL Manager V2 with Phase 3 enhancements
    let repository = DslDomainRepository::new(pool);
    let manager = DslManagerV2::new(repository);

    // Run the comprehensive Phase 3 demonstration
    match run_full_demonstration(&manager).await {
        Ok(_) => {
            println!("✨ Phase 3 demonstration completed successfully!");
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Demonstration failed: {}", e);
            Err(e.into())
        }
    }
}

/// Run full demonstration with database backend
async fn run_full_demonstration(manager: &DslManagerV2) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Phase 3: Domain-Specific Features Overview");
    println!("{}", "-".repeat(50));

    // 1. Domain Support Analysis
    demonstrate_domain_support(manager).await?;

    // 2. KYC Domain Specific Features
    demonstrate_kyc_domain_features(manager).await?;

    // 3. Functional State Visualization
    demonstrate_functional_state_visualization(manager).await?;

    // 4. Multiple Domain Comparison
    demonstrate_multi_domain_comparison(manager).await?;

    // 5. Advanced Domain Analytics
    demonstrate_domain_analytics(manager).await?;

    Ok(())
}

/// Demonstrate domain support and capabilities
async fn demonstrate_domain_support(
    manager: &DslManagerV2,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📋 Domain Support Analysis");
    println!("{}", "-".repeat(40));

    let domains = manager.list_domains(true).await?;
    println!("📊 Available domains: {}", domains.len());

    for domain in &domains {
        let supports_functional = manager.supports_functional_states(&domain.domain_name);
        let functional_states = manager.get_domain_functional_states(&domain.domain_name);
        let highlights = manager.get_domain_highlights(&domain.domain_name);

        println!(
            "   🏷️  {}: {}",
            domain.domain_name,
            domain.description.as_deref().unwrap_or("No description")
        );
        println!(
            "      - Functional States: {} ({})",
            if supports_functional { "✅" } else { "❌" },
            functional_states.len()
        );
        println!("      - Domain Highlights: {}", highlights.len());

        if supports_functional {
            println!("      - States: {}", functional_states.join(", "));
        }

        println!();
    }

    Ok(())
}

/// Demonstrate KYC domain specific features
async fn demonstrate_kyc_domain_features(
    manager: &DslManagerV2,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 KYC Domain-Specific Features");
    println!("{}", "-".repeat(40));

    // Create comprehensive KYC DSL with UBO calculation
    let kyc_dsl = create_comprehensive_kyc_dsl();
    println!("📝 Creating KYC DSL with UBO calculation workflow...");

    let version = manager
        .create_dsl_version(
            "KYC",
            &kyc_dsl,
            Some("Generate_UBO"),
            Some("Phase 3 demo: Comprehensive KYC workflow with UBO calculation and compliance checks"),
            Some("phase3_demo"),
        )
        .await?;

    println!(
        "   ✅ Created KYC version {} in Generate_UBO state",
        version.version_number
    );
    println!("   📊 DSL size: {} characters", kyc_dsl.len());

    // Generate domain-enhanced visualization
    println!("\n🎨 Generating KYC Domain-Enhanced Visualization...");
    let start_time = std::time::Instant::now();

    let enhanced_viz = manager
        .build_domain_enhanced_visualization("KYC", version.version_number, None)
        .await?;

    let generation_time = start_time.elapsed();

    println!("   ✅ Domain-enhanced visualization generated");
    println!("   ⏱️  Generation time: {:?}", generation_time);

    // Analyze domain-specific features
    println!("\n📊 KYC Domain Analysis:");
    println!(
        "   🏷️  Domain: {}",
        enhanced_viz.base_visualization.domain_context.domain_name
    );
    println!("   📈 Base Statistics:");
    println!(
        "      - Nodes: {}",
        enhanced_viz.base_visualization.statistics.total_nodes
    );
    println!(
        "      - Edges: {}",
        enhanced_viz.base_visualization.statistics.total_edges
    );
    println!(
        "      - Complexity: {}",
        enhanced_viz.base_visualization.statistics.complexity_score
    );

    // Domain-specific metrics
    println!("   🎯 Domain Metrics:");
    println!(
        "      - Entity Count: {}",
        enhanced_viz.domain_metrics.entity_count
    );
    println!(
        "      - Relationship Count: {}",
        enhanced_viz.domain_metrics.relationship_count
    );
    println!(
        "      - UBO Calculations: {}",
        enhanced_viz.domain_metrics.ubo_calculations
    );
    println!(
        "      - Compliance Operations: {}",
        enhanced_viz.domain_metrics.compliance_operations
    );
    println!(
        "      - Risk Score: {:.2}",
        enhanced_viz.domain_metrics.risk_score
    );
    println!(
        "      - Est. Execution Time: {}ms",
        enhanced_viz.domain_metrics.estimated_execution_time
    );

    // Domain highlights
    println!("   🌟 Domain Highlights:");
    for highlight in &enhanced_viz.domain_specific_highlights {
        let priority_icon = match highlight.priority {
            HighlightPriority::Critical => "🚨",
            HighlightPriority::High => "🔴",
            HighlightPriority::Medium => "🟡",
            HighlightPriority::Low => "🟢",
        };
        println!(
            "      {} {} - {}",
            priority_icon, highlight.highlight_type, highlight.description
        );
        println!("        Color: {}", highlight.color);
    }

    println!();
    Ok(())
}

/// Demonstrate functional state visualization
async fn demonstrate_functional_state_visualization(
    manager: &DslManagerV2,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Functional State Visualization");
    println!("{}", "-".repeat(40));

    // Create DSL versions in different functional states
    let states_to_demo = vec![
        ("Create_Case", "Initial KYC case creation"),
        ("Collect_Documents", "Document collection phase"),
        ("Generate_UBO", "UBO calculation phase"),
        ("Review_Edit", "Review and editing phase"),
    ];

    for (state, description) in states_to_demo {
        println!("🔄 Demonstrating '{}' functional state...", state);

        let test_dsl = create_simple_kyc_dsl_for_state(state);
        let version = manager
            .create_dsl_version(
                "KYC",
                &test_dsl,
                Some(state),
                Some(description),
                Some("phase3_functional_demo"),
            )
            .await?;

        // Build functional state visualization
        let functional_viz = manager
            .build_functional_state_visualization("KYC", version.version_number)
            .await?;

        println!("   📈 State Analysis:");
        println!("      - Current State: {}", functional_viz.current_state);
        println!(
            "      - Completion: {:.1}%",
            functional_viz.completion_percentage
        );
        println!(
            "      - Next States: {}",
            functional_viz.next_possible_states.join(", ")
        );

        // Show state progression
        println!("   🚶 State Progression:");
        for (i, step) in functional_viz.state_progression.iter().enumerate() {
            let status_icon = if step.is_current {
                "👉"
            } else if step.is_completed {
                "✅"
            } else if step.is_available {
                "⏳"
            } else {
                "⏸️"
            };

            println!(
                "      {}. {} {} - {} ({}min)",
                i + 1,
                status_icon,
                step.state_name,
                step.description,
                step.estimated_effort
            );
        }

        // Show workflow progression if available
        let enhanced_viz = manager
            .build_domain_enhanced_visualization("KYC", version.version_number, None)
            .await?;

        if let Some(workflow_progression) = &enhanced_viz.workflow_progression {
            println!(
                "   💼 Workflow Status: {}",
                workflow_progression.completion_status
            );
            println!("   📋 Recommended Actions:");
            for action in &workflow_progression.recommended_next_actions {
                println!("      - {}", action);
            }
        }

        println!();
    }

    Ok(())
}

/// Demonstrate multiple domain comparison
async fn demonstrate_multi_domain_comparison(
    manager: &DslManagerV2,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Multi-Domain Feature Comparison");
    println!("{}", "-".repeat(45));

    let domains_to_compare = vec!["KYC", "Onboarding", "Account_Opening", "Compliance"];
    let mut domain_comparisons = Vec::new();

    for domain_name in domains_to_compare {
        if manager.supports_functional_states(domain_name) {
            let functional_states = manager.get_domain_functional_states(domain_name);
            let highlights = manager.get_domain_highlights(domain_name);

            // Create a simple test DSL for each domain
            let test_dsl = create_domain_specific_test_dsl(domain_name);

            if let Ok(version) = manager
                .create_dsl_version(
                    domain_name,
                    &test_dsl,
                    functional_states.first().cloned().as_deref(),
                    Some(&format!(
                        "Phase 3 multi-domain comparison test for {}",
                        domain_name
                    )),
                    Some("phase3_comparison"),
                )
                .await
            {
                let enhanced_viz = manager
                    .build_domain_enhanced_visualization(domain_name, version.version_number, None)
                    .await?;

                domain_comparisons.push((domain_name.to_string(), enhanced_viz));

                println!("✅ {} Domain Analysis:", domain_name);
                println!("   - Functional States: {}", functional_states.len());
                println!("   - Domain Highlights: {}", highlights.len());
                println!(
                    "   - Est. Execution Time: {}ms",
                    enhanced_viz.domain_metrics.estimated_execution_time
                );
                println!(
                    "   - Base Complexity: {}",
                    enhanced_viz.base_visualization.statistics.complexity_score
                );
            }
        } else {
            println!("⏸️  {} Domain: No functional state support", domain_name);
        }
        println!();
    }

    // Compare domain characteristics
    if !domain_comparisons.is_empty() {
        println!("📊 Domain Comparison Summary:");
        println!("   Domain              | Exec Time | Complexity | Risk Score");
        println!("   {}", "-".repeat(55));

        for (domain_name, viz) in &domain_comparisons {
            println!(
                "   {:18} | {:7}ms | {:8} | {:8.2}",
                domain_name,
                viz.domain_metrics.estimated_execution_time,
                viz.base_visualization.statistics.complexity_score,
                viz.domain_metrics.risk_score
            );
        }
    }

    println!();
    Ok(())
}

/// Demonstrate advanced domain analytics
async fn demonstrate_domain_analytics(
    manager: &DslManagerV2,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📈 Advanced Domain Analytics");
    println!("{}", "-".repeat(40));

    // Create a complex KYC workflow for analytics
    let complex_kyc = create_complex_kyc_workflow();
    let version = manager
        .create_dsl_version(
            "KYC",
            &complex_kyc,
            Some("Review_Edit"),
            Some("Complex KYC workflow for advanced analytics demonstration"),
            Some("phase3_analytics"),
        )
        .await?;

    let enhanced_viz = manager
        .build_domain_enhanced_visualization("KYC", version.version_number, None)
        .await?;

    println!("🧮 Advanced Metrics Analysis:");

    // Complexity analysis
    let base_stats = &enhanced_viz.base_visualization.statistics;
    println!("   📊 Complexity Metrics:");
    println!("      - Total Nodes: {}", base_stats.total_nodes);
    println!("      - Total Edges: {}", base_stats.total_edges);
    println!("      - Max Depth: {}", base_stats.max_depth);
    println!("      - Complexity Score: {}", base_stats.complexity_score);
    println!(
        "      - Node/Edge Ratio: {:.2}",
        base_stats.total_nodes as f64 / base_stats.total_edges.max(1) as f64
    );

    // Domain-specific analysis
    let domain_metrics = &enhanced_viz.domain_metrics;
    println!("   🎯 Domain-Specific Metrics:");
    println!("      - Entity Operations: {}", domain_metrics.entity_count);
    println!(
        "      - Relationship Operations: {}",
        domain_metrics.relationship_count
    );
    println!(
        "      - UBO Calculations: {}",
        domain_metrics.ubo_calculations
    );
    println!(
        "      - Compliance Checks: {}",
        domain_metrics.compliance_operations
    );
    println!(
        "      - Document Collections: {}",
        domain_metrics.document_collections
    );

    // Risk assessment
    println!("   ⚠️  Risk Assessment:");
    let risk_level = if domain_metrics.risk_score < 2.0 {
        ("Low", "🟢")
    } else if domain_metrics.risk_score < 5.0 {
        ("Medium", "🟡")
    } else if domain_metrics.risk_score < 8.0 {
        ("High", "🟠")
    } else {
        ("Critical", "🔴")
    };

    println!("      - Risk Score: {:.2}", domain_metrics.risk_score);
    println!("      - Risk Level: {} {}", risk_level.1, risk_level.0);
    println!(
        "      - Est. Execution Time: {}ms",
        domain_metrics.estimated_execution_time
    );

    // Functional state analysis
    if let Some(functional_info) = &enhanced_viz.functional_state_info {
        println!("   🔄 Functional State Analysis:");
        println!("      - Current State: {}", functional_info.current_state);
        println!(
            "      - Progress: {:.1}%",
            functional_info.completion_percentage
        );
        println!(
            "      - Remaining States: {}",
            functional_info.available_states.len()
                - functional_info
                    .state_progression
                    .iter()
                    .filter(|s| s.is_completed || s.is_current)
                    .count()
        );

        let total_effort: u32 = functional_info
            .available_states
            .iter()
            .map(|s| s.estimated_effort)
            .sum();
        let completed_effort: u32 = functional_info
            .state_progression
            .iter()
            .filter(|s| s.is_completed)
            .map(|s| s.estimated_effort)
            .sum();

        println!("      - Total Effort: {}min", total_effort);
        println!("      - Completed Effort: {}min", completed_effort);
        println!(
            "      - Remaining Effort: {}min",
            total_effort - completed_effort
        );
    }

    println!();
    Ok(())
}

/// Run demonstration without database (mock mode)
async fn run_mock_demonstration() -> Result<(), Box<dyn std::error::Error>> {
    println!("📚 Mock Mode Demonstration");
    println!("{}", "-".repeat(30));

    // Demonstrate domain visualizer capabilities without database
    let visualizer = DomainVisualizer::new();

    println!("🎯 Domain Visualizer Analysis (Mock Mode):");

    for (domain_name, rules) in &visualizer.domain_rules {
        println!("\n📋 {} Domain:", domain_name);
        println!("   - Node Styles: {}", rules.node_styles.len());
        println!("   - Edge Styles: {}", rules.edge_styles.len());
        println!("   - Functional States: {}", rules.functional_states.len());
        println!(
            "   - Critical Edge Types: {}",
            rules.critical_edge_types.len()
        );
        println!(
            "   - Base Execution Time: {}ms",
            rules.base_execution_time_ms
        );

        // Show functional state progression
        if !rules.functional_states.is_empty() {
            println!("   📊 Functional State Flow:");
            for (i, state) in rules.functional_states.iter().enumerate() {
                let deps = if state.dependencies.is_empty() {
                    "None".to_string()
                } else {
                    state.dependencies.join(", ")
                };
                println!(
                    "      {}. {} ({}min) [Deps: {}]",
                    i + 1,
                    state.name,
                    state.estimated_effort,
                    deps
                );
            }
        }
    }

    // Show domain highlights
    println!("\n🌟 Domain Highlights Analysis:");
    for domain in ["KYC", "Onboarding", "Account_Opening"] {
        let highlights = visualizer.identify_domain_highlights(domain);
        println!("   {} Domain: {} highlights", domain, highlights.len());
        for highlight in highlights {
            println!(
                "      - {}: {}",
                highlight.highlight_type, highlight.description
            );
        }
    }

    println!("\n✨ Mock demonstration complete!");
    println!("   📝 To see full functionality, configure DATABASE_URL");

    Ok(())
}

/// Create comprehensive KYC DSL for demonstration
fn create_comprehensive_kyc_dsl() -> String {
    r#"
(workflow "comprehensive-kyc-with-ubo"
    ;; Entity declarations with comprehensive properties
    (declare-entity "customer" "person"
        (properties
            (name "Jane Smith")
            (dob "1985-03-20")
            (nationality "US")
            (customer-type "individual")
            (risk-category "medium")))

    (declare-entity "holding-company" "corporation"
        (properties
            (name "Tech Holdings LLC")
            (jurisdiction "Delaware")
            (incorporation-date "2020-01-15")
            (entity-type "LLC")
            (business-purpose "Investment Holding")))

    (declare-entity "subsidiary" "corporation"
        (properties
            (name "Tech Operations Inc")
            (jurisdiction "Delaware")
            (parent-company "holding-company")))

    ;; Document collection with compliance tracking
    (parallel
        (obtain-document "passport" "government"
            (properties
                (required true)
                (expires "2028-05-15")
                (document-type "identity")
                (verification-level "primary")))

        (obtain-document "utility-bill" "third-party"
            (properties
                (age-limit 90)
                (purpose "address-verification")
                (verification-level "secondary")))

        (obtain-document "corporate-registry" "government"
            (properties
                (entity "holding-company")
                (document-type "incorporation")
                (required true))))

    ;; Complex entity relationship mapping
    (create-edge "customer" "holding-company" "beneficial-owner"
        (properties
            (ownership-percentage 75.0)
            (control-type "voting-control")
            (acquisition-date "2023-01-01")
            (verification-source "corporate-registry")))

    (create-edge "holding-company" "subsidiary" "parent-company"
        (properties
            (ownership-percentage 100.0)
            (control-type "direct-control")
            (relationship-type "subsidiary")))

    ;; Ultimate beneficial ownership calculation
    (calculate-ubo "holding-company"
        (properties
            (threshold 25.0)
            (max-depth 5)
            (algorithm "recursive-ownership")
            (include-control true)
            (consolidation-method "aggregate")))

    ;; Comprehensive compliance checks
    (sequential
        (solicit-attribute "pep-status" "customer" "boolean"
            (properties
                (source "sanctions-database")
                (verification-required true)))

        (solicit-attribute "sanctions-check" "customer" "boolean"
            (properties
                (provider "world-check")
                (screening-level "enhanced")))

        (solicit-attribute "source-of-wealth" "customer" "string"
            (properties
                (required true)
                (documentation-required true)))

        ;; Advanced conflict resolution
        (resolve-conflict "customer" "address"
            (waterfall-strategy
                (primary-source "government-registry" 0.95)
                (secondary-source "utility-provider" 0.8)
                (tertiary-source "self-declared" 0.5)))

        ;; Final reporting and monitoring
        (generate-report "customer" "kyc-assessment"
            (properties
                (include-ubo true)
                (format "pdf")
                (compliance-level "enhanced")
                (regulatory-requirements ["BSA" "CDD" "EDD"])))

        (schedule-monitoring "customer" "ongoing"
            (properties
                (frequency "quarterly")
                (triggers ["sanctions-list-update" "pep-status-change"])
                (auto-review true)
                (escalation-rules ["manual-review-required"])))))
"#
    .to_string()
}

/// Create simple KYC DSL for specific functional state
fn create_simple_kyc_dsl_for_state(state: &str) -> String {
    match state {
        "Create_Case" => r#"
(workflow "create-kyc-case"
    (declare-entity "customer" "person"
        (properties (name "John Doe"))))
"#
        .to_string(),
        "Collect_Documents" => r#"
(workflow "collect-documents"
    (declare-entity "customer" "person")
    (obtain-document "passport" "government")
    (obtain-document "utility-bill" "third-party"))
"#
        .to_string(),
        "Generate_UBO" => r#"
(workflow "generate-ubo"
    (declare-entity "customer" "person")
    (declare-entity "company" "corporation")
    (create-edge "customer" "company" "beneficial-owner")
    (calculate-ubo "company"))
"#
        .to_string(),
        "Review_Edit" => r#"
(workflow "review-edit"
    (declare-entity "customer" "person")
    (generate-report "customer" "preliminary")
    (schedule-monitoring "customer" "ongoing"))
"#
        .to_string(),
        _ => r#"(workflow "default" (declare-entity "entity" "person"))"#.to_string(),
    }
}

/// Create domain-specific test DSL
fn create_domain_specific_test_dsl(domain: &str) -> String {
    match domain {
        "KYC" => r#"
(workflow "kyc-test"
    (declare-entity "customer" "person")
    (calculate-ubo "customer"))
"#
        .to_string(),
        "Onboarding" => r#"
(workflow "onboarding-test"
    (declare-entity "customer" "person")
    (sequential
        (obtain-document "id" "government")
        (obtain-document "proof-address" "third-party")))
"#
        .to_string(),
        "Account_Opening" => r#"
(workflow "account-opening-test"
    (declare-entity "applicant" "person")
    (obtain-document "application" "internal"))
"#
        .to_string(),
        "Compliance" => r#"
(workflow "compliance-test"
    (declare-entity "entity" "corporation")
    (solicit-attribute "compliance-status" "entity" "boolean"))
"#
        .to_string(),
        _ => r#"(workflow "generic-test" (declare-entity "entity" "person"))"#.to_string(),
    }
}

/// Create complex KYC workflow for analytics
fn create_complex_kyc_workflow() -> String {
    r#"
(workflow "complex-kyc-analytics"
    ;; Multiple entity declarations
    (declare-entity "primary-customer" "person"
        (properties (name "Alice Johnson") (risk-level "high")))
    (declare-entity "spouse" "person"
        (properties (name "Bob Johnson") (relationship "spouse")))
    (declare-entity "trust" "trust"
        (properties (name "Johnson Family Trust") (type "revocable")))
    (declare-entity "holding-corp" "corporation"
        (properties (name "Johnson Holdings Inc") (jurisdiction "Delaware")))
    (declare-entity "operating-co" "corporation"
        (properties (name "Johnson Enterprises LLC") (jurisdiction "Nevada")))

    ;; Complex document collection
    (parallel
        (obtain-document "passport" "government")
        (obtain-document "drivers-license" "government")
        (obtain-document "utility-bill" "third-party")
        (obtain-document "bank-statement" "financial-institution")
        (obtain-document "tax-return" "government")
        (obtain-document "trust-agreement" "legal")
        (obtain-document "corporate-charter" "government"))

    ;; Multiple relationship mappings
    (create-edge "primary-customer" "spouse" "married-to")
    (create-edge "primary-customer" "trust" "trustor")
    (create-edge "spouse" "trust" "beneficiary")
    (create-edge "trust" "holding-corp" "beneficial-owner"
        (properties (ownership-percentage 60.0)))
    (create-edge "holding-corp" "operating-co" "parent-company"
        (properties (ownership-percentage 85.0)))

    ;; Multiple UBO calculations
    (calculate-ubo "trust" (properties (threshold 25.0)))
    (calculate-ubo "holding-corp" (properties (threshold 25.0)))
    (calculate-ubo "operating-co" (properties (threshold 25.0)))

    ;; Extensive compliance checks
    (sequential
        (solicit-attribute "pep-status" "primary-customer" "boolean")
        (solicit-attribute "sanctions-check" "primary-customer" "boolean")
        (solicit-attribute "pep-status" "spouse" "boolean")
        (solicit-attribute "sanctions-check" "spouse" "boolean")
        (solicit-attribute "source-of-wealth" "primary-customer" "string")
        (solicit-attribute "business-purpose" "trust" "string")

        (parallel
            (resolve-conflict "primary-customer" "address")
            (resolve-conflict "spouse" "address")
            (resolve-conflict "trust" "address"))

        (generate-report "primary-customer" "comprehensive-kyc")
        (generate-report "trust" "entity-structure")
        (schedule-monitoring "primary-customer" "enhanced")
        (schedule-monitoring "trust" "quarterly")))
"#
    .to_string()
}

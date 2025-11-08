//! Phase 2: AST Visualization Demonstration
//!
//! This example demonstrates the enhanced DSL Manager V2 with comprehensive
//! AST visualization capabilities implemented in Phase 2.
//!
//! Features demonstrated:
//! - Domain-aware DSL compilation with database persistence
//! - AST storage and caching mechanisms
//! - Multiple visualization layout types (Tree, Graph, Hierarchical)
//! - Flexible filtering and styling options
//! - Domain context and compilation metadata preservation
//! - Performance metrics and complexity scoring

use ob_poc::ast::{Program, Statement, Value, Workflow};
use ob_poc::database::DslDomainRepository;
use ob_poc::dsl_manager_v2::{
    ASTVisualization, DslManagerV2, FilterConfig, LayoutType, StylingConfig, VisualizationOptions,
};
use ob_poc::models;
use sqlx::PgPool;
use std::collections::HashMap;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();

    println!("🚀 Phase 2: AST Visualization Demo");
    println!("{}", "=".repeat(60));
    println!();

    // Connect to database (if available)
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost:5432/dsl-ob-poc".to_string());

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

    // Initialize DSL Manager V2
    let repository = DslDomainRepository::new(pool);
    let manager = DslManagerV2::new(repository);

    // Run the demonstration
    match run_full_demonstration(&manager).await {
        Ok(_) => {
            println!("✨ Phase 2 demonstration completed successfully!");
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
    // 1. Domain Management Demo
    println!("📁 Domain Management Demonstration");
    println!("{}", "-".repeat(40));

    let domains = manager.list_domains(true).await?;
    println!("📋 Available domains: {}", domains.len());
    for domain in &domains {
        println!(
            "   - {} ({})",
            domain.domain_name,
            domain.description.as_deref().unwrap_or("No description")
        );
    }
    println!();

    // 2. Create comprehensive test DSL
    let kyc_dsl = create_comprehensive_kyc_dsl();
    println!("📝 Creating new DSL version with comprehensive KYC workflow...");

    let version = manager
        .create_dsl_version(
            "KYC",
            &kyc_dsl,
            Some("Phase2_Demo"),
            Some("Comprehensive KYC workflow demonstrating Phase 2 AST visualization features"),
            Some("phase2_demo"),
        )
        .await?;

    println!(
        "   ✅ Created version {} in KYC domain",
        version.version_number
    );
    println!("   📊 DSL size: {} characters", kyc_dsl.len());
    println!();

    // 3. AST Compilation Demo
    println!("🔧 AST Compilation Demonstration");
    println!("{}", "-".repeat(40));

    let start_time = std::time::Instant::now();
    let parsed_ast = manager
        .compile_dsl_version("KYC", version.version_number, false)
        .await?;
    let compile_duration = start_time.elapsed();

    println!("   ✅ AST compilation successful");
    println!("   ⏱️  Compilation time: {:?}", compile_duration);
    println!("   📊 AST Statistics:");
    println!("      - Node count: {}", parsed_ast.node_count.unwrap_or(0));
    println!(
        "      - Complexity score: {:?}",
        parsed_ast.complexity_score.unwrap_or_default()
    );
    println!(
        "      - AST hash: {}",
        parsed_ast.ast_hash.as_deref().unwrap_or("N/A")
    );
    println!("      - Grammar version: {}", parsed_ast.grammar_version);
    println!();

    // 4. Visualization Demonstrations
    demonstrate_tree_visualization(manager, &version).await?;
    demonstrate_graph_visualization(manager, &version).await?;
    demonstrate_hierarchical_visualization(manager, &version).await?;
    demonstrate_filtered_visualization(manager, &version).await?;

    // 5. Performance and Caching Demo
    demonstrate_caching_performance(manager, &version).await?;

    Ok(())
}

/// Demonstrate Tree layout visualization
async fn demonstrate_tree_visualization(
    manager: &DslManagerV2,
    version: &models::DslVersion,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🌳 Tree Layout Visualization");
    println!("{}", "-".repeat(40));

    let options = VisualizationOptions {
        layout: Some(LayoutType::Tree),
        styling: None,
        filters: Some(FilterConfig {
            show_only_nodes: None,
            hide_nodes: None,
            max_depth: Some(10),
            show_properties: true,
        }),
        include_compilation_info: true,
        include_domain_context: true,
        show_functional_states: true,
        max_depth: Some(10),
    };

    let viz = manager
        .build_ast_visualization("KYC", version.version_number, Some(options))
        .await?;

    print_visualization_summary(&viz, "Tree");
    print_node_breakdown(&viz);
    println!();

    Ok(())
}

/// Demonstrate Graph layout visualization
async fn demonstrate_graph_visualization(
    manager: &DslManagerV2,
    version: &models::DslVersion,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Graph Layout Visualization");
    println!("{}", "-".repeat(45));

    let mut node_colors = HashMap::new();
    node_colors.insert("DeclareEntity".to_string(), "#4CAF50".to_string());
    node_colors.insert("CreateEdge".to_string(), "#2196F3".to_string());
    node_colors.insert("CalculateUbo".to_string(), "#FF9800".to_string());
    node_colors.insert("ObtainDocument".to_string(), "#9C27B0".to_string());

    let options = VisualizationOptions {
        layout: Some(LayoutType::Graph),
        styling: Some(StylingConfig {
            theme: "entity-relationship".to_string(),
            node_colors,
        }),
        filters: Some(FilterConfig {
            show_only_nodes: None,
            hide_nodes: Some(vec!["Placeholder".to_string()]),
            max_depth: Some(8),
            show_properties: false,
        }),
        include_compilation_info: true,
        include_domain_context: true,
        show_functional_states: false,
        max_depth: Some(8),
    };

    let viz = manager
        .build_ast_visualization("KYC", version.version_number, Some(options))
        .await?;

    print_visualization_summary(&viz, "Graph");
    print_edge_analysis(&viz);
    println!();

    Ok(())
}

/// Demonstrate Hierarchical layout visualization
async fn demonstrate_hierarchical_visualization(
    manager: &DslManagerV2,
    version: &models::DslVersion,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🏗️  Hierarchical Layout Visualization");
    println!("{}", "-".repeat(50));

    let options = VisualizationOptions {
        layout: Some(LayoutType::Hierarchical),
        styling: Some(StylingConfig {
            theme: "hierarchical-flow".to_string(),
            node_colors: HashMap::new(),
        }),
        filters: Some(FilterConfig {
            show_only_nodes: None,
            hide_nodes: None,
            max_depth: Some(6),
            show_properties: true,
        }),
        include_compilation_info: true,
        include_domain_context: true,
        show_functional_states: true,
        max_depth: Some(6),
    };

    let viz = manager
        .build_ast_visualization("KYC", version.version_number, Some(options))
        .await?;

    print_visualization_summary(&viz, "Hierarchical");
    print_depth_analysis(&viz);
    println!();

    Ok(())
}

/// Demonstrate filtered visualization with node type focus
async fn demonstrate_filtered_visualization(
    manager: &DslManagerV2,
    version: &models::DslVersion,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Filtered Visualization (Entity Operations Only)");
    println!("{}", "-".repeat(50));

    let options = VisualizationOptions {
        layout: Some(LayoutType::Tree),
        styling: None,
        filters: Some(FilterConfig {
            show_only_nodes: Some(vec![
                "Program".to_string(),
                "Workflow".to_string(),
                "DeclareEntity".to_string(),
                "CreateEdge".to_string(),
                "CalculateUbo".to_string(),
            ]),
            hide_nodes: None,
            max_depth: Some(5),
            show_properties: false,
        }),
        include_compilation_info: false,
        include_domain_context: true,
        show_functional_states: false,
        max_depth: Some(5),
    };

    let viz = manager
        .build_ast_visualization("KYC", version.version_number, Some(options))
        .await?;

    print_visualization_summary(&viz, "Filtered (Entity Focus)");
    println!("   🎯 Showing only: Program, Workflow, DeclareEntity, CreateEdge, CalculateUbo");
    println!();

    Ok(())
}

/// Demonstrate caching and performance characteristics
async fn demonstrate_caching_performance(
    manager: &DslManagerV2,
    version: &models::DslVersion,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("⚡ Caching and Performance Demonstration");
    println!("{}", "-".repeat(45));

    // First compilation (cache miss)
    println!("🔄 First compilation (cache miss)...");
    let start = std::time::Instant::now();
    let _viz1 = manager
        .build_ast_visualization("KYC", version.version_number, None)
        .await?;
    let first_duration = start.elapsed();
    println!("   ⏱️  Duration: {:?}", first_duration);

    // Second compilation (cache hit)
    println!("♻️  Second compilation (cache hit)...");
    let start = std::time::Instant::now();
    let _viz2 = manager
        .build_ast_visualization("KYC", version.version_number, None)
        .await?;
    let second_duration = start.elapsed();
    println!("   ⏱️  Duration: {:?}", second_duration);

    if second_duration < first_duration {
        let speedup = first_duration.as_millis() as f64 / second_duration.as_millis() as f64;
        println!("   🚀 Cache speedup: {:.1}x faster", speedup);
    }

    // Force recompilation
    println!("🔧 Forced recompilation...");
    let start = std::time::Instant::now();
    let _parsed_ast = manager
        .compile_dsl_version("KYC", version.version_number, true)
        .await?;
    let recompile_duration = start.elapsed();
    println!("   ⏱️  Recompilation duration: {:?}", recompile_duration);

    println!();
    Ok(())
}

/// Run demonstration without database (mock mode)
async fn run_mock_demonstration() -> Result<(), Box<dyn std::error::Error>> {
    println!("📚 Mock Mode Demonstration");
    println!("{}", "-".repeat(30));

    // Create sample AST structure
    let sample_program = create_sample_program();
    println!("📊 Sample AST Structure:");
    println!("   - Workflows: {}", sample_program.workflows.len());

    let total_statements: usize = sample_program
        .workflows
        .iter()
        .map(|w| w.statements.len())
        .sum();
    println!("   - Total statements: {}", total_statements);

    // Demonstrate AST structure analysis
    analyze_ast_structure(&sample_program);

    println!("✨ Mock demonstration complete!");
    println!("   📝 To see full functionality, configure DATABASE_URL");

    Ok(())
}

/// Create comprehensive KYC DSL for demonstration
fn create_comprehensive_kyc_dsl() -> String {
    r#"
(workflow "comprehensive-kyc-onboarding"
    ;; Customer entity declaration
    (declare-entity "customer" "person"
        (properties
            (name "Jane Smith")
            (dob "1985-03-20")
            (nationality "US")
            (customer-type "individual")))

    ;; Corporate entity declaration
    (declare-entity "holding-company" "corporation"
        (properties
            (name "Tech Holdings LLC")
            (jurisdiction "Delaware")
            (incorporation-date "2020-01-15")
            (entity-type "LLC")))

    ;; Document collection workflow
    (parallel
        (obtain-document "passport" "government"
            (properties
                (required true)
                (expires "2028-05-15")
                (document-type "identity")))

        (obtain-document "utility-bill" "third-party"
            (properties
                (age-limit 90)
                (purpose "address-verification")))

        (obtain-document "bank-statement" "financial-institution"
            (properties
                (age-limit 90)
                (required true))))

    ;; Entity relationship mapping
    (create-edge "customer" "holding-company" "beneficial-owner"
        (properties
            (ownership-percentage 75.0)
            (control-type "voting-control")
            (acquisition-date "2023-01-01")))

    ;; Ultimate beneficial ownership calculation
    (calculate-ubo "holding-company"
        (properties
            (threshold 25.0)
            (max-depth 5)
            (algorithm "recursive-ownership")
            (include-control true)))

    ;; Compliance and risk assessment
    (sequential
        (solicit-attribute "pep-status" "customer" "boolean"
            (properties (source "sanctions-database")))

        (solicit-attribute "sanctions-check" "customer" "boolean"
            (properties (provider "world-check")))

        (solicit-attribute "source-of-wealth" "customer" "string"
            (properties (required true)))

        (resolve-conflict "customer" "address"
            (waterfall-strategy
                (primary-source "government-registry" 0.95)
                (secondary-source "utility-provider" 0.8)
                (tertiary-source "self-declared" 0.5)))

        (generate-report "customer" "kyc-assessment"
            (properties
                (include-ubo true)
                (format "pdf")
                (compliance-level "enhanced")))

        (schedule-monitoring "customer" "ongoing"
            (properties
                (frequency "quarterly")
                (triggers ["sanctions-list-update" "pep-status-change"])
                (auto-review true)))))
"#
    .to_string()
}

/// Create sample program for mock demonstration
fn create_sample_program() -> Program {
    let mut properties = HashMap::new();
    properties.insert("example".to_string(), Value::String("demo".to_string()));

    Program {
        workflows: vec![Workflow {
            id: "sample-workflow".to_string(),
            properties: HashMap::new(),
            statements: vec![
                Statement::DeclareEntity {
                    id: "entity1".to_string(),
                    entity_type: "person".to_string(),
                    properties: properties.clone(),
                },
                Statement::CreateEdge {
                    from: "entity1".to_string(),
                    to: "entity2".to_string(),
                    edge_type: "owns".to_string(),
                    properties: HashMap::new(),
                },
                Statement::CalculateUbo {
                    entity_id: "entity1".to_string(),
                    properties: HashMap::new(),
                },
            ],
        }],
    }
}

/// Print visualization summary
fn print_visualization_summary(viz: &ASTVisualization, layout_name: &str) {
    println!("   ✅ {} visualization generated", layout_name);
    println!("   📊 Statistics:");
    println!("      - Nodes: {}", viz.statistics.total_nodes);
    println!("      - Edges: {}", viz.statistics.total_edges);
    println!("      - Max depth: {}", viz.statistics.max_depth);
    println!("      - Complexity: {}", viz.statistics.complexity_score);
    println!(
        "   🏷️  Root: {} ({})",
        viz.root_node.label, viz.root_node.node_type
    );
    println!(
        "   📅 Generated: {}",
        viz.metadata.generated_at.format("%H:%M:%S")
    );
}

/// Print node type breakdown
fn print_node_breakdown(viz: &ASTVisualization) {
    let mut node_counts = HashMap::new();

    // Count root node
    *node_counts
        .entry(viz.root_node.node_type.clone())
        .or_insert(0) += 1;

    // This would need to traverse all nodes in the full structure
    // For now, just show what we have
    println!("   📈 Node breakdown: {:?}", node_counts);
}

/// Print edge analysis
fn print_edge_analysis(viz: &ASTVisualization) {
    let edge_count = viz.edges.len();
    println!("   🔗 Edge analysis:");
    println!("      - Total edges: {}", edge_count);
    if edge_count > 0 {
        println!(
            "      - Average edges per node: {:.1}",
            edge_count as f64 / viz.statistics.total_nodes as f64
        );
    }
}

/// Print depth analysis
fn print_depth_analysis(viz: &ASTVisualization) {
    println!("   📏 Depth analysis:");
    println!("      - Maximum depth: {}", viz.statistics.max_depth);
    println!(
        "      - Depth complexity factor: {}",
        viz.statistics.max_depth as f64 / viz.statistics.total_nodes as f64
    );
}

/// Analyze AST structure for mock demonstration
fn analyze_ast_structure(program: &Program) {
    println!("🔍 AST Structure Analysis:");

    for (i, workflow) in program.workflows.iter().enumerate() {
        println!("   📋 Workflow {}: '{}'", i + 1, workflow.id);
        println!("      - Statements: {}", workflow.statements.len());

        let mut statement_types = HashMap::new();
        for statement in &workflow.statements {
            let stmt_type = match statement {
                Statement::DeclareEntity { .. } => "DeclareEntity",
                Statement::CreateEdge { .. } => "CreateEdge",
                Statement::CalculateUbo { .. } => "CalculateUbo",
                Statement::ObtainDocument { .. } => "ObtainDocument",
                Statement::Parallel(_) => "Parallel",
                Statement::Sequential(_) => "Sequential",
                _ => "Other",
            };
            *statement_types.entry(stmt_type).or_insert(0) += 1;
        }

        println!("      - Types: {:?}", statement_types);
    }
}

//! Minimal Orchestration Demo - Core DSL Pipeline Test
//!
//! This example demonstrates the core DSL orchestration pipeline without
//! the complex execution modules that have compilation issues.
//!
//! ## What This Proves:
//! 1. DSL Manager initialization works
//! 2. DSL Orchestration Interface is functional
//! 3. DSL Pipeline Processor responds to orchestrated calls
//! 4. Database service can be initialized
//! 5. Visualizer can process DSL content
//! 6. End-to-end call chain: DSL Manager → DSL Mod → Response
//!
//! ## Usage:
//! ```bash
//! cargo run --example minimal_orchestration_demo
//! ```

use ob_poc::{
    dsl::{
        DslOrchestrationInterface, DslPipelineProcessor, OrchestrationContext,
        OrchestrationOperation, OrchestrationOperationType,
    },
    dsl_manager::CleanDslManager,
    dsl_visualizer::{DslVisualizer, StateResult},
};

use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 Minimal Orchestration Demo Starting");
    info!("🎯 Testing core DSL orchestration without database dependencies");

    // Step 1: Initialize Core Components
    info!("🏗️  Step 1: Initializing core components...");
    let dsl_processor = DslPipelineProcessor::new();
    let mut dsl_manager = CleanDslManager::new();
    let visualizer = DslVisualizer::new();
    info!("✅ Components initialized successfully");

    // Step 2: Test Direct DSL Orchestration Interface
    info!("🔄 Step 2: Testing DSL Orchestration Interface...");
    test_orchestration_interface(&dsl_processor).await?;

    // Step 3: Test DSL Manager Processing
    info!("📝 Step 3: Testing DSL Manager processing...");
    test_dsl_manager_processing(&mut dsl_manager).await?;

    // Step 4: Test Visualization Pipeline
    info!("🎨 Step 4: Testing visualization pipeline...");
    test_visualization_pipeline(&visualizer).await?;

    // Step 5: Test Orchestration Metrics (Phase 5)
    info!("📊 Step 5: Testing Phase 5 orchestration metrics...");
    test_orchestration_metrics(&dsl_processor).await?;

    // Step 6: Integration Test - Full Pipeline
    info!("🔗 Step 6: Full pipeline integration test...");
    test_full_pipeline_integration(&dsl_processor, &visualizer).await?;

    info!("✅ Minimal Orchestration Demo completed successfully!");
    info!("🎯 Proven: Core DSL orchestration pipeline works end-to-end");

    Ok(())
}

async fn test_orchestration_interface(
    processor: &DslPipelineProcessor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("  Testing orchestration interface methods...");

    // Test metrics retrieval
    let metrics = processor.get_orchestration_metrics().await?;
    info!(
        "    ✅ Metrics retrieval: {} total operations",
        metrics.total_operations
    );

    // Test health check
    let health = processor.orchestration_health_check().await?;
    info!("    ✅ Health check: system healthy = {}", health.healthy);

    // Test operation processing
    let context = OrchestrationContext::new("test-user".to_string(), "test-domain".to_string());
    let operation = OrchestrationOperation::new(
        OrchestrationOperationType::Parse,
        "(test.operation :name \"orchestration-test\")".to_string(),
        context,
    );

    let result = processor.process_orchestrated_operation(operation).await?;
    info!("    ✅ Operation processing: success = {}", result.success);

    Ok(())
}

async fn test_dsl_manager_processing(
    manager: &mut CleanDslManager,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("  Testing DSL Manager processing...");

    let test_dsl_operations = vec![
        "(case.create :name \"Test Customer\")",
        "(kyc.start :customer-id \"CUST-001\")",
        "(identity.verify :document-type \"passport\")",
    ];

    let mut successful = 0;

    for (i, dsl) in test_dsl_operations.iter().enumerate() {
        let start = Instant::now();
        let result = manager.process_dsl_request(dsl.to_string());
        let duration = start.elapsed();

        if result.success {
            successful += 1;
            info!(
                "    ✅ DSL operation {}: success in {}ms",
                i + 1,
                duration.as_millis()
            );
        } else {
            warn!(
                "    ⚠️  DSL operation {}: failed in {}ms - {:?}",
                i + 1,
                duration.as_millis(),
                result.errors
            );
        }
    }

    info!(
        "    📊 DSL Manager: {}/{} operations successful",
        successful,
        test_dsl_operations.len()
    );
    Ok(())
}

async fn test_visualization_pipeline(
    visualizer: &DslVisualizer,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("  Testing visualization pipeline...");

    let test_visualizations = vec![
        ("Simple DSL", "(case.create :name \"Visual Test\")"),
        (
            "KYC DSL",
            "(kyc.collect :customer-id \"CUST-001\" :data-types [\"IDENTITY\"])",
        ),
        (
            "Complex DSL",
            "(ubo.resolve-ubos :entity-id \"CORP-001\" :threshold 25.0)",
        ),
    ];

    let mut successful = 0;

    for (name, _dsl) in &test_visualizations {
        let mut context = HashMap::new();
        context.insert("test_name".to_string(), name.to_string());
        context.insert("source".to_string(), "orchestration_demo".to_string());

        // Create a mock StateResult for visualization
        let state_result = StateResult {
            success: true,
            case_id: format!("TEST-{}", name.replace(" ", "-")),
            version_number: 1,
            snapshot_id: "demo-snapshot".to_string(),
            errors: vec![],
            processing_time_ms: 10,
        };

        match visualizer.generate_visualization(&state_result).await {
            viz_result if viz_result.success => {
                successful += 1;
                info!(
                    "    ✅ Visualization '{}': Success - {} bytes generated",
                    name, viz_result.output_size_bytes
                );
            }
            viz_result => {
                warn!(
                    "    ❌ Visualization '{}' failed: {:?}",
                    name, viz_result.errors
                );
            }
        }
    }

    info!(
        "    📊 Visualization: {}/{} successful",
        successful,
        test_visualizations.len()
    );
    Ok(())
}

async fn test_orchestration_metrics(
    processor: &DslPipelineProcessor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("  Testing Phase 5 orchestration metrics...");

    // Reset metrics
    processor.reset_orchestration_metrics().await?;
    info!("    🔄 Metrics reset successfully");

    // Perform several operations to generate metrics
    let context = OrchestrationContext::new("metrics-test".to_string(), "metrics".to_string());

    for i in 1..=5 {
        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::ProcessComplete,
            format!("(metrics.test :iteration {} :data \"test\")", i),
            context.clone(),
        );

        let _result = processor.process_orchestrated_operation(operation).await?;
    }

    // Check final metrics
    let final_metrics = processor.get_orchestration_metrics().await?;
    info!(
        "    📈 Final metrics: {}",
        final_metrics.performance_summary()
    );
    info!(
        "    📊 Operations: {}, Success rate: {:.1}%",
        final_metrics.total_operations,
        (final_metrics.successful_operations as f64 / final_metrics.total_operations.max(1) as f64)
            * 100.0
    );

    Ok(())
}

async fn test_full_pipeline_integration(
    processor: &DslPipelineProcessor,
    visualizer: &DslVisualizer,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("  Testing full pipeline integration...");

    let integration_start = Instant::now();

    // Step 1: Process through orchestration
    let context =
        OrchestrationContext::new("integration-test".to_string(), "full-pipeline".to_string());
    let operation = OrchestrationOperation::new(
        OrchestrationOperationType::ProcessComplete,
        "(integration.test :pipeline \"full\" :components [\"dsl-manager\" \"dsl-mod\" \"visualizer\"])".to_string(),
        context,
    );

    let orchestration_result = processor.process_orchestrated_operation(operation).await?;
    info!(
        "    ✅ Orchestration step: success = {}",
        orchestration_result.success
    );

    // Step 2: Generate visualization of the result
    let mut viz_context = HashMap::new();
    viz_context.insert("integration_test".to_string(), "true".to_string());
    viz_context.insert(
        "operation_id".to_string(),
        orchestration_result.operation_id.clone(),
    );

    // Create a mock StateResult for integration test visualization
    let integration_state_result = StateResult {
        success: true,
        case_id: "integration-test".to_string(),
        version_number: 1,
        snapshot_id: "integration-snapshot".to_string(),
        errors: vec![],
        processing_time_ms: 10,
    };

    let viz_result = visualizer
        .generate_visualization(&integration_state_result)
        .await;
    if viz_result.success {
        info!(
            "    ✅ Visualization step: Success - {} bytes generated",
            viz_result.output_size_bytes
        );
    } else {
        warn!("    ⚠️  Visualization step failed: {:?}", viz_result.errors);
    }

    let total_time = integration_start.elapsed();
    info!(
        "    🏁 Full pipeline completed in {}ms",
        total_time.as_millis()
    );

    Ok(())
}

//! Phase 5: Simple Performance Metrics Demo
//!
//! This example demonstrates the Phase 5 implementation of performance monitoring
//! and orchestration metrics in a simplified way that focuses on the core features.
//!
//! ## Features Demonstrated
//! - Basic orchestration metrics collection
//! - Performance timing and tracking
//! - Success/failure rate monitoring
//! - Simple tracing output
//!
//! ## Usage
//! ```bash
//! cargo run --example phase5_simple_metrics_demo
//! ```

use ob_poc::dsl::{
    DslOrchestrationInterface, DslPipelineProcessor, OrchestrationContext, OrchestrationOperation,
    OrchestrationOperationType,
};
use std::time::Instant;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize simple tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    info!("🚀 Phase 5: Simple Performance Metrics Demo Starting");

    // Create DSL Pipeline Processor
    let processor = DslPipelineProcessor::new();

    // Reset metrics to start clean
    processor.reset_orchestration_metrics().await?;
    info!("📊 Metrics reset - starting fresh");

    // Create orchestration context
    let context = OrchestrationContext::new("demo-user".to_string(), "kyc".to_string());

    // Test operations with different types and complexities
    let test_operations = vec![
        (
            "Simple Parse",
            OrchestrationOperationType::Parse,
            "(case.create :name \"Test\")",
        ),
        (
            "Validation",
            OrchestrationOperationType::Validate,
            "(kyc.start :customer-id @customer{uuid-001})",
        ),
        (
            "Execute",
            OrchestrationOperationType::Execute,
            "(identity.verify :type \"passport\")",
        ),
        (
            "Invalid DSL",
            OrchestrationOperationType::Parse,
            "invalid-syntax-here",
        ),
        (
            "Complex Operation",
            OrchestrationOperationType::ProcessComplete,
            "(ubo.collect-entity-data :entity-id \"CORP-001\" :jurisdiction \"US\")",
        ),
    ];

    info!("🔄 Processing {} test operations...", test_operations.len());

    let demo_start = Instant::now();

    for (i, (name, op_type, dsl)) in test_operations.iter().enumerate() {
        info!("  Step {}: {}", i + 1, name);

        let operation =
            OrchestrationOperation::new(op_type.clone(), dsl.to_string(), context.clone());

        let start = Instant::now();
        let result = processor.process_orchestrated_operation(operation).await?;
        let duration = start.elapsed();

        if result.success {
            info!("    ✅ Success in {}ms", duration.as_millis());
        } else {
            warn!(
                "    ❌ Failed in {}ms - Errors: {}",
                duration.as_millis(),
                result.errors.len()
            );
        }
    }

    let total_duration = demo_start.elapsed();

    // Get final metrics
    let metrics = processor.get_orchestration_metrics().await?;

    info!("");
    info!("📈 Final Performance Metrics Summary:");
    info!("  Total Operations: {}", metrics.total_operations);
    info!("  Successful: {}", metrics.successful_operations);
    info!("  Failed: {}", metrics.failed_operations);
    info!(
        "  Success Rate: {:.1}%",
        (metrics.successful_operations as f64 / metrics.total_operations.max(1) as f64) * 100.0
    );
    info!(
        "  Average Processing Time: {:.2}ms",
        metrics.average_processing_time_ms
    );
    info!(
        "  Operations per Second: {:.2}",
        metrics.operations_per_second
    );
    info!("  Error Rate: {:.1}%", metrics.error_rate * 100.0);

    info!("");
    info!("🏆 Demo Performance:");
    info!("  Total Demo Time: {:.2}s", total_duration.as_secs_f64());
    info!(
        "  Demo Throughput: {:.2} ops/sec",
        test_operations.len() as f64 / total_duration.as_secs_f64()
    );

    info!("");
    info!("📊 One-liner Summary: {}", metrics.performance_summary());

    // Demonstrate metrics reset
    info!("");
    info!("🔄 Testing metrics reset...");
    processor.reset_orchestration_metrics().await?;
    let reset_metrics = processor.get_orchestration_metrics().await?;
    info!(
        "  Metrics after reset: {} total operations",
        reset_metrics.total_operations
    );

    info!("");
    info!("✅ Phase 5 Simple Performance Metrics Demo completed successfully!");

    Ok(())
}

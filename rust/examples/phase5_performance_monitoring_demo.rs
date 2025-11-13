//! Phase 5: Performance Monitoring and Orchestration Demo
//!
//! This example demonstrates the Phase 5 implementation of performance monitoring
//! and orchestration metrics in the DSL Manager → DSL Mod pipeline.
//!
//! ## Features Demonstrated
//! - Real-time orchestration metrics collection
//! - Tracing and instrumentation
//! - Performance monitoring across the call chain
//! - Concurrent operation tracking
//! - Cache hit rate and error rate monitoring
//! - System resource monitoring
//!
//! ## Usage
//! ```bash
//! cargo run --example phase5_performance_monitoring_demo
//! ```

use ob_poc::{
    dsl::{
        DslOrchestrationInterface, DslPipelineProcessor, OrchestrationContext,
        OrchestrationOperation, OrchestrationOperationType, ProcessingOptions,
    },
    dsl_manager::{CleanDslManager, CleanManagerConfig},
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for Phase 5 instrumentation
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🚀 Starting Phase 5 Performance Monitoring Demo");

    // Create DSL Pipeline Processor with orchestration metrics
    let dsl_processor = Arc::new(DslPipelineProcessor::new());

    // Create DSL Manager for orchestration
    let manager_config = CleanManagerConfig::default();
    let dsl_manager = CleanDslManager::new(manager_config);

    info!("📊 Phase 5 Demo: Testing orchestration performance monitoring");

    // Test 1: Basic metrics collection
    info!("\n=== Test 1: Basic Orchestration Metrics ===");
    demonstrate_basic_metrics(&dsl_processor).await?;

    // Test 2: Concurrent operations monitoring
    info!("\n=== Test 2: Concurrent Operations Monitoring ===");
    demonstrate_concurrent_operations(&dsl_processor).await?;

    // Test 3: Performance benchmarking
    info!("\n=== Test 3: Performance Benchmarking ===");
    demonstrate_performance_benchmarking(&dsl_processor).await?;

    // Test 4: Error rate and cache monitoring
    info!("\n=== Test 4: Error Rate and Cache Monitoring ===");
    demonstrate_error_and_cache_monitoring(&dsl_processor).await?;

    // Test 5: Real-time metrics dashboard simulation
    info!("\n=== Test 5: Real-time Metrics Dashboard ===");
    demonstrate_real_time_dashboard(&dsl_processor).await?;

    info!("✅ Phase 5 Performance Monitoring Demo completed successfully!");
    Ok(())
}

/// Test 1: Demonstrate basic orchestration metrics collection
async fn demonstrate_basic_metrics(
    processor: &DslPipelineProcessor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing basic orchestration metrics collection...");

    // Reset metrics to start clean
    processor.reset_orchestration_metrics().await?;

    let context = OrchestrationContext::new("demo-user".to_string(), "kyc".to_string());

    // Create several test operations
    let test_operations = vec![
        "(case.create :customer-name \"John Doe\")",
        "(kyc.start :customer-id @customer{uuid-001})",
        "(identity.verify :document-type \"passport\")",
        "(case.approve :case-id \"CASE-001\")",
    ];

    for (i, dsl) in test_operations.iter().enumerate() {
        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::ProcessComplete,
            dsl.to_string(),
            context.clone(),
        )
        .with_priority((i % 3 + 1) as u8 * 3); // Vary priorities

        let start = Instant::now();
        let result = processor.process_orchestrated_operation(operation).await?;
        let duration = start.elapsed();

        info!(
            "Operation {}: {} ({}ms) - Success: {}",
            i + 1,
            &dsl[0..30.min(dsl.len())],
            duration.as_millis(),
            result.success
        );
    }

    // Display metrics
    let metrics = processor.get_orchestration_metrics().await?;
    info!("📈 Basic Metrics Summary:");
    info!("  Total Operations: {}", metrics.total_operations);
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

    Ok(())
}

/// Test 2: Demonstrate concurrent operations monitoring
async fn demonstrate_concurrent_operations(
    processor: &DslPipelineProcessor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing concurrent operations monitoring...");

    processor.reset_orchestration_metrics().await?;

    let context = OrchestrationContext::new("concurrent-user".to_string(), "ubo".to_string());

    // Create multiple concurrent operations
    let mut handles = Vec::new();

    for i in 0..5 {
        let processor_clone = processor.clone();
        let context_clone = context.clone();

        let handle = tokio::spawn(async move {
            let operation = OrchestrationOperation::new(
                OrchestrationOperationType::Execute,
                format!("(ubo.collect-entity-data :entity-id \"ENTITY-{:03}\")", i),
                context_clone,
            );

            // Add some artificial delay to simulate real work
            sleep(Duration::from_millis(50 + i * 20)).await;

            processor_clone
                .process_orchestrated_operation(operation)
                .await
        });

        handles.push(handle);
    }

    info!("⏳ Processing {} concurrent operations...", handles.len());

    // Wait for all operations to complete
    let mut successful = 0;
    let mut failed = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok(result)) => {
                if result.success {
                    successful += 1;
                } else {
                    failed += 1;
                }
            }
            _ => failed += 1,
        }
    }

    let metrics = processor.get_orchestration_metrics().await?;
    info!("🔄 Concurrent Operations Results:");
    info!("  Successful: {}", successful);
    info!("  Failed: {}", failed);
    info!("  Total Operations: {}", metrics.total_operations);
    info!(
        "  Average Latency: {:.2}ms",
        metrics.orchestration_latency_ms
    );

    Ok(())
}

/// Test 3: Demonstrate performance benchmarking
async fn demonstrate_performance_benchmarking(
    processor: &DslPipelineProcessor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing performance benchmarking...");

    processor.reset_orchestration_metrics().await?;

    let context = OrchestrationContext::new("bench-user".to_string(), "isda".to_string());
    let benchmark_iterations = 20;

    info!(
        "🏃 Running {} iterations for performance benchmark...",
        benchmark_iterations
    );

    let overall_start = Instant::now();

    for i in 0..benchmark_iterations {
        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::Parse,
            format!(
                "(isda.execute_trade :trade-id \"TRADE-{:03}\" :counterparty \"BANK-A\")",
                i
            ),
            context.clone(),
        );

        processor.process_orchestrated_operation(operation).await?;

        // Print progress every 5 iterations
        if (i + 1) % 5 == 0 {
            debug!("  Completed {}/{} iterations", i + 1, benchmark_iterations);
        }
    }

    let total_time = overall_start.elapsed();
    let metrics = processor.get_orchestration_metrics().await?;

    info!("🏆 Performance Benchmark Results:");
    info!("  Total Time: {:.2}s", total_time.as_secs_f64());
    info!("  Operations: {}", benchmark_iterations);
    info!(
        "  Throughput: {:.2} ops/sec",
        benchmark_iterations as f64 / total_time.as_secs_f64()
    );
    info!(
        "  Average Operation Time: {:.2}ms",
        metrics.average_processing_time_ms
    );
    info!("  Peak Memory Usage: {} bytes", metrics.peak_memory_bytes);

    Ok(())
}

/// Test 4: Demonstrate error rate and cache monitoring
async fn demonstrate_error_and_cache_monitoring(
    processor: &DslPipelineProcessor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Testing error rate and cache monitoring...");

    processor.reset_orchestration_metrics().await?;

    let context =
        OrchestrationContext::new("error-test-user".to_string(), "compliance".to_string());

    // Mix of valid and invalid operations to test error rates
    let test_cases = vec![
        ("(compliance.screen :customer-id @customer{uuid-001})", true),
        ("invalid-dsl-syntax", false),
        ("(compliance.monitor :entity-type \"CORP\")", true),
        ("(unclosed-paren", false),
        ("(compliance.assess :risk-score 85)", true),
        ("", false),
        ("(compliance.report :type \"UBO\")", true),
    ];

    for (i, (dsl, should_succeed)) in test_cases.iter().enumerate() {
        let operation = OrchestrationOperation::new(
            OrchestrationOperationType::Validate,
            dsl.to_string(),
            context.clone(),
        );

        let result = processor.process_orchestrated_operation(operation).await?;

        let status = if result.success { "✅" } else { "❌" };
        debug!(
            "Test case {}: {} - Expected: {}, Actual: {}",
            i + 1,
            status,
            if *should_succeed {
                "Success"
            } else {
                "Failure"
            },
            if result.success { "Success" } else { "Failure" }
        );
    }

    let metrics = processor.get_orchestration_metrics().await?;

    info!("📊 Error Rate and Cache Monitoring Results:");
    info!("  Total Operations: {}", metrics.total_operations);
    info!("  Success Rate: {:.1}%", (1.0 - metrics.error_rate) * 100.0);
    info!("  Error Rate: {:.1}%", metrics.error_rate * 100.0);
    info!("  Cache Hit Rate: {:.1}%", metrics.cache_hit_rate * 100.0);
    info!(
        "  Database Operations: {}",
        metrics.database_operations_count
    );

    Ok(())
}

/// Test 5: Simulate a real-time metrics dashboard
async fn demonstrate_real_time_dashboard(
    processor: &DslPipelineProcessor,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Simulating real-time metrics dashboard...");

    processor.reset_orchestration_metrics().await?;

    let context =
        OrchestrationContext::new("dashboard-user".to_string(), "multi-domain".to_string());

    info!("📺 Starting real-time dashboard simulation (10 seconds)...");

    let dashboard_start = Instant::now();
    let mut iteration = 0;

    while dashboard_start.elapsed() < Duration::from_secs(10) {
        iteration += 1;

        // Simulate mixed workload
        let domains = ["kyc", "ubo", "isda", "compliance"];
        let domain = domains[iteration % domains.len()];

        let operation = OrchestrationOperation::new(
            match iteration % 4 {
                0 => OrchestrationOperationType::Parse,
                1 => OrchestrationOperationType::Validate,
                2 => OrchestrationOperationType::Execute,
                _ => OrchestrationOperationType::ProcessComplete,
            },
            format!("({}.operation :id \"DASH-{:03}\")", domain, iteration),
            context.clone(),
        );

        processor.process_orchestrated_operation(operation).await?;

        // Display dashboard every 2 seconds
        if iteration % 5 == 0 {
            let metrics = processor.get_orchestration_metrics().await?;

            info!("📊 Real-time Dashboard Update #{}", iteration / 5);
            info!(
                "  ⚡ Operations: {} | Success: {:.1}% | Avg Time: {:.1}ms",
                metrics.total_operations,
                (metrics.successful_operations as f64 / metrics.total_operations.max(1) as f64)
                    * 100.0,
                metrics.average_processing_time_ms
            );
            info!(
                "  🚀 Throughput: {:.1} ops/sec | Concurrent: {} | Queue: {}",
                metrics.operations_per_second, metrics.concurrent_operations, metrics.queue_depth
            );
            info!(
                "  💾 Memory: {} bytes | Peak: {} bytes | DB Ops: {}",
                metrics.memory_usage_bytes,
                metrics.peak_memory_bytes,
                metrics.database_operations_count
            );
        }

        sleep(Duration::from_millis(400)).await;
    }

    let final_metrics = processor.get_orchestration_metrics().await?;

    info!("🏁 Final Dashboard Summary:");
    info!("{}", final_metrics.performance_summary());
    info!(
        "  Total Runtime: {:.1}s",
        dashboard_start.elapsed().as_secs_f64()
    );

    Ok(())
}

/// Helper trait to add Clone to DslPipelineProcessor (for concurrent testing)
trait ProcessorClone {
    fn clone(&self) -> Self;
}

impl ProcessorClone for DslPipelineProcessor {
    fn clone(&self) -> Self {
        // Create a new processor for concurrent operations
        // In a real implementation, this would share the metrics
        DslPipelineProcessor::new()
    }
}

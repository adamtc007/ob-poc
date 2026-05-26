//! Performance smoke test: 100 instances through InMemoryJourneyStore.
//!
//! Marked `#[ignore]` — run with `--include-ignored` for perf testing:
//! ```bash
//! cargo test -p bpmn-test-harness -- --include-ignored smoke_100
//! ```
//!
//! A full 1,000-instance test against `PostgresJourneyStore` is deferred until
//! DB integration tests are wired (requires `DATABASE_URL`).

use bpmn_runtime::{
    InMemoryJourneyStore, InstanceStatus, RuntimeEngine, ScriptedAdaptor, VerbRegistry,
};
use std::sync::Arc;

const LINEAR_DSL: &str = r#"
(node start-1 :kind start-event)
(node task-1  :kind service-task)
(node end-1   :kind end-event)
(flow start-1 -> task-1)
(flow task-1  -> end-1)
"#;

/// 100 instances run sequentially through the InMemoryJourneyStore.
///
/// Asserts:
/// - All 100 instances complete.
/// - Wall time is under 30 seconds (very conservative — typical <500 ms).
#[tokio::test]
#[ignore]
async fn smoke_100_instances_in_memory() {
    let spec = Arc::new(bpmn_test_harness::compile_dsl(LINEAR_DSL));

    let start = std::time::Instant::now();
    let mut completed = 0usize;

    for i in 0..100u64 {
        let store = Arc::new(InMemoryJourneyStore::new());
        let engine = RuntimeEngine::new(
            store,
            Arc::clone(&spec),
            Arc::new(VerbRegistry::new()),
            Arc::new(ScriptedAdaptor::new()),
        );

        let instance_id = engine
            .start_instance(serde_json::json!({"i": i}))
            .await
            .expect("start_instance failed");

        let status = engine
            .get_instance_status(instance_id)
            .await
            .expect("get_status failed")
            .expect("instance not found");

        if status == InstanceStatus::Completed {
            completed += 1;
        }
    }

    let elapsed = start.elapsed();
    println!(
        "smoke_100_instances_in_memory: {}/100 completed in {:?} ({:.1} ms/instance)",
        completed,
        elapsed,
        elapsed.as_millis() as f64 / 100.0,
    );

    assert_eq!(completed, 100, "all 100 instances should complete");
    assert!(
        elapsed.as_secs() < 30,
        "100 instances should complete within 30 s"
    );
}

/// Metrics accumulate correctly over 10 instances on a shared engine.
#[tokio::test]
#[ignore]
async fn smoke_metrics_10_instances_shared_engine() {
    let spec = Arc::new(bpmn_test_harness::compile_dsl(LINEAR_DSL));
    let store = Arc::new(InMemoryJourneyStore::new());
    let engine = RuntimeEngine::new(
        store,
        spec,
        Arc::new(VerbRegistry::new()),
        Arc::new(ScriptedAdaptor::new()),
    );

    for i in 0..10u64 {
        engine
            .start_instance(serde_json::json!({"i": i}))
            .await
            .expect("start_instance failed");
    }

    let snap = engine.metrics().snapshot();
    assert_eq!(snap.instances_started, 10);
    assert_eq!(snap.instances_completed, 10);
    assert_eq!(snap.instances_failed, 0);
    assert!(snap.events_processed >= 10);

    println!("Prometheus output:\n{}", engine.metrics().prometheus_text());
}

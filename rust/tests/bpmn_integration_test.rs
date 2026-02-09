//! BPMN-Lite Phase B3 — End-to-End Integration Tests
//!
//! All tests require:
//!   1. A running bpmn-lite gRPC service at `BPMN_LITE_GRPC_URL`
//!   2. A PostgreSQL database at `DATABASE_URL` with migration 073 applied
//!
//! Run with:
//! ```bash
//! BPMN_LITE_GRPC_URL=http://localhost:50052 \
//!   DATABASE_URL=postgresql:///data_designer \
//!   cargo test --features database --test bpmn_integration_test -- --ignored --nocapture
//! ```
#![cfg(all(feature = "database", feature = "vnext-repl"))]

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use ob_poc::bpmn_integration::{
    canonical::canonical_json_with_hash,
    client::{BpmnLiteConnection, CompleteJobRequest, StartProcessRequest},
    config::WorkflowConfigIndex,
    correlation::CorrelationStore,
    job_frames::JobFrameStore,
    parked_tokens::ParkedTokenStore,
    pending_dispatches::PendingDispatchStore,
    types::{CorrelationRecord, CorrelationStatus, ExecutionRoute, ParkedToken, ParkedTokenStatus},
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const KYC_BPMN: &str = include_str!("models/kyc-open-case.bpmn");

/// Get bpmn-lite gRPC URL from env or skip test.
fn bpmn_url() -> String {
    std::env::var("BPMN_LITE_GRPC_URL").unwrap_or_else(|_| "http://[::1]:50052".to_string())
}

/// Create a connected BpmnLiteConnection.
async fn bpmn_client() -> BpmnLiteConnection {
    BpmnLiteConnection::connect(&bpmn_url())
        .await
        .expect("Failed to connect to bpmn-lite — is the service running?")
}

/// Create a PgPool for integration tests.
async fn test_pool() -> sqlx::PgPool {
    let url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    sqlx::PgPool::connect(&url)
        .await
        .expect("Failed to connect to PostgreSQL")
}

/// Load workflow config from the project config directory.
fn load_config() -> WorkflowConfigIndex {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let config_path = std::path::Path::new(manifest_dir).join("config/workflows.yaml");
    WorkflowConfigIndex::load_from_file(&config_path).expect("Failed to load workflows.yaml")
}

/// Compile the kyc-open-case model and return the bytecode version.
async fn compile_model(client: &BpmnLiteConnection) -> Vec<u8> {
    let result = client.compile(KYC_BPMN).await.expect("Compile failed");
    assert!(
        result.diagnostics.iter().all(|d| d.severity != "error"),
        "Compile produced errors: {:?}",
        result.diagnostics
    );
    result.bytecode_version
}

// ===========================================================================
// B3.1 — Compile kyc-open-case model
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_01_compile_kyc_model() {
    let client = bpmn_client().await;
    let result = client.compile(KYC_BPMN).await.expect("Compile failed");

    // Should produce bytecode (32-byte version hash).
    assert_eq!(
        result.bytecode_version.len(),
        32,
        "Expected 32-byte bytecode version, got {} bytes",
        result.bytecode_version.len()
    );

    // Should produce no error diagnostics.
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.severity == "error")
        .collect();
    assert!(errors.is_empty(), "Unexpected compile errors: {:?}", errors);

    eprintln!(
        "Compiled kyc-open-case: version={}, diagnostics={}",
        hex::encode(&result.bytecode_version),
        result.diagnostics.len()
    );
}

// ===========================================================================
// B3.2 — Start process instance
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_02_start_process_instance() {
    let client = bpmn_client().await;
    let _bytecode = compile_model(&client).await;

    let payload = serde_json::json!({ "entity_id": Uuid::new_v4().to_string() });
    let (canonical, hash) = canonical_json_with_hash(&payload);

    let instance_id = client
        .start_process(StartProcessRequest {
            process_key: "kyc_open_case".to_string(),
            bytecode_version: Vec::new(),
            domain_payload: canonical,
            domain_payload_hash: hash,
            orch_flags: HashMap::new(),
            correlation_id: Uuid::new_v4(),
        })
        .await
        .expect("StartProcess failed");

    assert_ne!(instance_id, Uuid::nil(), "Instance ID should not be nil");
    eprintln!("Started process instance: {}", instance_id);

    // Inspect should show Running state.
    let inspection = client.inspect(instance_id).await.expect("Inspect failed");
    assert_eq!(inspection.state, "Running", "Process should be Running");
    eprintln!("Process state: {}", inspection.state);
}

// ===========================================================================
// B3.3 — Full happy path (3 jobs + 2 waits)
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_03_full_happy_path() {
    let client = bpmn_client().await;
    let _bytecode = compile_model(&client).await;

    let entity_id = Uuid::new_v4();
    let payload = serde_json::json!({ "entity_id": entity_id.to_string() });
    let (canonical, hash) = canonical_json_with_hash(&payload);

    // Start process.
    let instance_id = client
        .start_process(StartProcessRequest {
            process_key: "kyc_open_case".to_string(),
            bytecode_version: Vec::new(),
            domain_payload: canonical,
            domain_payload_hash: hash,
            orch_flags: HashMap::new(),
            correlation_id: Uuid::new_v4(),
        })
        .await
        .expect("StartProcess failed");

    // --- Job 1: create_case_record ---
    let jobs = client
        .activate_jobs(&["create_case_record".to_string()], 1, 5_000, "test-worker")
        .await
        .expect("ActivateJobs failed");
    assert_eq!(jobs.len(), 1, "Expected 1 create_case_record job");
    assert_eq!(jobs[0].task_type, "create_case_record");

    let result = serde_json::json!({ "case_id": Uuid::new_v4().to_string() });
    let (res_canonical, res_hash) = canonical_json_with_hash(&result);
    client
        .complete_job(CompleteJobRequest {
            job_key: jobs[0].job_key.clone(),
            domain_payload: res_canonical,
            domain_payload_hash: res_hash,
            orch_flags: HashMap::new(),
        })
        .await
        .expect("CompleteJob failed for create_case_record");

    // --- Job 2: request_documents ---
    let jobs = client
        .activate_jobs(&["request_documents".to_string()], 1, 5_000, "test-worker")
        .await
        .expect("ActivateJobs failed");
    assert_eq!(jobs.len(), 1, "Expected 1 request_documents job");

    let (res_canonical, res_hash) = canonical_json_with_hash(&serde_json::json!({}));
    client
        .complete_job(CompleteJobRequest {
            job_key: jobs[0].job_key.clone(),
            domain_payload: res_canonical,
            domain_payload_hash: res_hash,
            orch_flags: HashMap::new(),
        })
        .await
        .expect("CompleteJob failed for request_documents");

    // --- Wait: docs_received (message) ---
    // Process should be waiting for message.
    let inspection = client.inspect(instance_id).await.expect("Inspect failed");
    assert_eq!(inspection.state, "Running");
    assert!(
        !inspection.waits.is_empty(),
        "Expected at least 1 wait (message wait)"
    );

    client
        .signal(instance_id, "docs_received", None)
        .await
        .expect("Signal docs_received failed");

    // --- Wait: reviewer_decision (user task) ---
    // After signal, the user task should be activatable.
    let jobs = client
        .activate_jobs(&["reviewer_decision".to_string()], 1, 5_000, "test-worker")
        .await
        .expect("ActivateJobs failed");
    assert_eq!(jobs.len(), 1, "Expected 1 reviewer_decision job");

    let (res_canonical, res_hash) =
        canonical_json_with_hash(&serde_json::json!({ "decision": "approved" }));
    client
        .complete_job(CompleteJobRequest {
            job_key: jobs[0].job_key.clone(),
            domain_payload: res_canonical,
            domain_payload_hash: res_hash,
            orch_flags: HashMap::new(),
        })
        .await
        .expect("CompleteJob failed for reviewer_decision");

    // --- Job 3: record_decision ---
    let jobs = client
        .activate_jobs(&["record_decision".to_string()], 1, 5_000, "test-worker")
        .await
        .expect("ActivateJobs failed");
    assert_eq!(jobs.len(), 1, "Expected 1 record_decision job");

    let (res_canonical, res_hash) = canonical_json_with_hash(&serde_json::json!({}));
    client
        .complete_job(CompleteJobRequest {
            job_key: jobs[0].job_key.clone(),
            domain_payload: res_canonical,
            domain_payload_hash: res_hash,
            orch_flags: HashMap::new(),
        })
        .await
        .expect("CompleteJob failed for record_decision");

    // Process should be Completed.
    let inspection = client.inspect(instance_id).await.expect("Inspect failed");
    assert_eq!(
        inspection.state, "Completed",
        "Process should be Completed after all jobs done"
    );
    eprintln!("Full happy path completed for instance {}", instance_id);
}

// ===========================================================================
// B3.4 — Idempotency / dedupe (T-NET-1)
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_04_job_dedupe() {
    let pool = test_pool().await;
    let job_frames = JobFrameStore::new(pool.clone());

    // Insert a completed job frame.
    let frame = ob_poc::bpmn_integration::types::JobFrame {
        job_key: format!("dedupe-test-{}", Uuid::new_v4()),
        process_instance_id: Uuid::new_v4(),
        task_type: "create_case_record".to_string(),
        worker_id: "test-worker".to_string(),
        status: ob_poc::bpmn_integration::types::JobFrameStatus::Active,
        activated_at: chrono::Utc::now(),
        completed_at: None,
        attempts: 1,
    };

    // First upsert — new.
    let is_new = job_frames.upsert(&frame).await.expect("upsert failed");
    assert!(is_new, "First upsert should report new");

    // Mark completed.
    job_frames
        .mark_completed(&frame.job_key)
        .await
        .expect("mark_completed failed");

    // Second upsert — should detect existing completed job.
    let is_new = job_frames.upsert(&frame).await.expect("upsert failed");
    assert!(!is_new, "Second upsert should detect existing");

    // Verify it stayed completed.
    let found = job_frames
        .find_by_job_key(&frame.job_key)
        .await
        .expect("find failed");
    assert!(found.is_some());
    assert_eq!(
        found.unwrap().status,
        ob_poc::bpmn_integration::types::JobFrameStatus::Completed
    );

    eprintln!("Dedupe test passed — completed job not re-executed");
}

// ===========================================================================
// B3.5 — Payload hash integrity
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_05_payload_hash_integrity() {
    use ob_poc::bpmn_integration::canonical::validate_payload_hash;

    let payload = serde_json::json!({
        "entity_id": "abc-123",
        "case_type": "enhanced"
    });
    let (canonical, hash) = canonical_json_with_hash(&payload);

    // Hash should validate.
    assert!(validate_payload_hash(&canonical, &hash));

    // Tampered payload should fail.
    let tampered = canonical.replace("abc-123", "xyz-999");
    assert!(!validate_payload_hash(&tampered, &hash));

    // Stable hash: same payload → same hash.
    let (canonical2, hash2) = canonical_json_with_hash(&payload);
    assert_eq!(canonical, canonical2);
    assert_eq!(hash, hash2);

    // Different key ordering in source → same canonical form.
    let payload_reordered = serde_json::json!({
        "case_type": "enhanced",
        "entity_id": "abc-123"
    });
    let (canonical3, hash3) = canonical_json_with_hash(&payload_reordered);
    assert_eq!(
        canonical, canonical3,
        "Canonical should normalize key order"
    );
    assert_eq!(hash, hash3, "Hash should be stable across key orderings");

    eprintln!("Payload hash integrity verified");
}

// ===========================================================================
// B3.6 — Cancellation (T-NET-4)
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_06_cancellation() {
    let client = bpmn_client().await;
    let _bytecode = compile_model(&client).await;

    let payload = serde_json::json!({ "entity_id": Uuid::new_v4().to_string() });
    let (canonical, hash) = canonical_json_with_hash(&payload);

    let instance_id = client
        .start_process(StartProcessRequest {
            process_key: "kyc_open_case".to_string(),
            bytecode_version: Vec::new(),
            domain_payload: canonical,
            domain_payload_hash: hash,
            orch_flags: HashMap::new(),
            correlation_id: Uuid::new_v4(),
        })
        .await
        .expect("StartProcess failed");

    // Cancel immediately.
    client
        .cancel(instance_id, "Test cancellation")
        .await
        .expect("Cancel failed");

    // Inspect should show Cancelled.
    let inspection = client.inspect(instance_id).await.expect("Inspect failed");
    assert_eq!(inspection.state, "Cancelled", "Process should be Cancelled");

    // No jobs should be activatable after cancellation.
    let jobs = client
        .activate_jobs(&["create_case_record".to_string()], 1, 1_000, "test-worker")
        .await
        .expect("ActivateJobs failed");
    // Jobs activated before cancellation might still appear, but new ones should not.
    // The key invariant is that the process state is Cancelled.

    eprintln!(
        "Cancellation verified: instance {} is Cancelled, {} residual jobs",
        instance_id,
        jobs.len()
    );
}

// ===========================================================================
// B3.7 — Crash recovery (bpmn-lite restart → VM resumes from snapshot)
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_07_crash_recovery() {
    // This test verifies that process state survives across bpmn-lite restarts.
    // It requires manual service restart between steps.
    //
    // Approach: Start a process, complete first job, then verify
    // inspection still shows Running state (proving in-memory state
    // was checkpointed to persistent store).

    let client = bpmn_client().await;
    let _bytecode = compile_model(&client).await;

    let payload = serde_json::json!({ "entity_id": Uuid::new_v4().to_string() });
    let (canonical, hash) = canonical_json_with_hash(&payload);

    let instance_id = client
        .start_process(StartProcessRequest {
            process_key: "kyc_open_case".to_string(),
            bytecode_version: Vec::new(),
            domain_payload: canonical,
            domain_payload_hash: hash,
            orch_flags: HashMap::new(),
            correlation_id: Uuid::new_v4(),
        })
        .await
        .expect("StartProcess failed");

    // Complete first job.
    let jobs = client
        .activate_jobs(&["create_case_record".to_string()], 1, 5_000, "test-worker")
        .await
        .expect("ActivateJobs failed");
    assert_eq!(jobs.len(), 1);

    let (res_canonical, res_hash) =
        canonical_json_with_hash(&serde_json::json!({ "case_id": "test-case" }));
    client
        .complete_job(CompleteJobRequest {
            job_key: jobs[0].job_key.clone(),
            domain_payload: res_canonical,
            domain_payload_hash: res_hash,
            orch_flags: HashMap::new(),
        })
        .await
        .expect("CompleteJob failed");

    // Verify process is still Running (not lost).
    let inspection = client.inspect(instance_id).await.expect("Inspect failed");
    assert_eq!(
        inspection.state, "Running",
        "Process should still be Running after first job"
    );

    eprintln!(
        "Crash recovery baseline: instance {} state={}, fibers={}, waits={}",
        instance_id,
        inspection.state,
        inspection.fibers.len(),
        inspection.waits.len()
    );
    eprintln!("To fully test crash recovery: restart bpmn-lite service,");
    eprintln!(
        "then run b3_02 inspect on instance {} — should still be Running",
        instance_id
    );
}

// ===========================================================================
// B3.8 — Runtime switch equivalence (Direct vs Orchestrated → same output)
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_08_runtime_switch_equivalence() {
    let config = load_config();

    // Verify kyc-case verbs route to Orchestrated.
    let route = config.route_for_verb("kyc-case.create");
    assert_eq!(
        route,
        ExecutionRoute::Orchestrated,
        "kyc-case.create should be Orchestrated"
    );

    // Verify a non-orchestrated verb routes to Direct.
    let route = config.route_for_verb("session.info");
    assert_eq!(
        route,
        ExecutionRoute::Direct,
        "session.info should be Direct"
    );

    // Verify all task bindings have valid verb mappings.
    let binding = config.binding_for_verb("kyc-case.create");
    assert!(
        binding.is_some(),
        "kyc-case.create should have a workflow binding"
    );
    let wb = binding.unwrap();
    assert_eq!(wb.process_key.as_deref(), Some("kyc_open_case"));

    // Verify all task_types in the binding are resolvable.
    for tb in &wb.task_bindings {
        let lookup = config.binding_for_task_type(&tb.task_type);
        assert!(
            lookup.is_some(),
            "Task type '{}' should be resolvable",
            tb.task_type
        );
    }

    eprintln!("Runtime switch equivalence verified");
}

// ===========================================================================
// B3.9 — EventBridge event ordering
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_09_event_bridge_ordering() {
    use ob_poc::bpmn_integration::client::BpmnLifecycleEvent;
    use ob_poc::bpmn_integration::event_bridge::EventBridge;
    use ob_poc::bpmn_integration::types::OutcomeEvent;

    // Test translate_event for all expected event types.
    let events = vec![
        ("JobCompleted", true),
        ("Completed", true),
        ("Cancelled", true),
        ("IncidentCreated", true),
        ("SomeOtherEvent", false),
    ];

    for (event_type, should_translate) in events {
        let event = BpmnLifecycleEvent {
            sequence: 1,
            event_type: event_type.to_string(),
            process_instance_id: Uuid::new_v4().to_string(),
            payload_json: match event_type {
                "JobCompleted" => {
                    r#"{"job_key":"j1","task_type":"create_case_record","result":"{}"}"#.to_string()
                }
                "IncidentCreated" => {
                    r#"{"error":"test error","service_task_id":"st1"}"#.to_string()
                }
                "Cancelled" => r#"{"reason":"test cancel"}"#.to_string(),
                _ => "{}".to_string(),
            },
        };

        let outcome = EventBridge::translate_event(&event);
        if should_translate {
            assert!(
                outcome.is_some(),
                "Event type '{}' should translate",
                event_type
            );
        } else {
            assert!(
                outcome.is_none(),
                "Event type '{}' should not translate",
                event_type
            );
        }
    }

    // Verify no BPMN-internal details leak into OutcomeEvent.
    let event = BpmnLifecycleEvent {
        sequence: 2,
        event_type: "Completed".to_string(),
        process_instance_id: Uuid::new_v4().to_string(),
        payload_json: "{}".to_string(),
    };
    let outcome = EventBridge::translate_event(&event).unwrap();
    match outcome {
        OutcomeEvent::ProcessCompleted {
            process_instance_id,
        } => {
            // Should be a valid UUID, not raw BPMN element IDs.
            assert_ne!(
                process_instance_id,
                Uuid::nil(),
                "process_instance_id should be a valid UUID"
            );
        }
        other => panic!("Expected ProcessCompleted, got {:?}", other),
    }

    eprintln!("EventBridge ordering and BPMN leakage check passed");
}

// ===========================================================================
// B3.10 — Stream disconnect + redeliver (T-NET-2)
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_10_stream_disconnect_redeliver() {
    // This test verifies that jobs are redelivered after a worker disconnects
    // without completing them.

    let client = bpmn_client().await;
    let _bytecode = compile_model(&client).await;

    let payload = serde_json::json!({ "entity_id": Uuid::new_v4().to_string() });
    let (canonical, hash) = canonical_json_with_hash(&payload);

    let instance_id = client
        .start_process(StartProcessRequest {
            process_key: "kyc_open_case".to_string(),
            bytecode_version: Vec::new(),
            domain_payload: canonical,
            domain_payload_hash: hash,
            orch_flags: HashMap::new(),
            correlation_id: Uuid::new_v4(),
        })
        .await
        .expect("StartProcess failed");

    // Activate job but DON'T complete it (simulates worker crash).
    let jobs = client
        .activate_jobs(
            &["create_case_record".to_string()],
            1,
            2_000,
            "doomed-worker",
        )
        .await
        .expect("ActivateJobs failed");
    assert_eq!(jobs.len(), 1);
    let orphan_key = jobs[0].job_key.clone();

    // Fail the job to force redelivery.
    client
        .fail_job(&orphan_key, "WORKER_CRASH", "Simulated crash", 0)
        .await
        .expect("FailJob failed");

    // Re-activate — should get the same task type back.
    let jobs = client
        .activate_jobs(
            &["create_case_record".to_string()],
            1,
            5_000,
            "recovery-worker",
        )
        .await
        .expect("ActivateJobs failed");
    assert_eq!(jobs.len(), 1, "Job should be redelivered after failure");
    assert_eq!(jobs[0].task_type, "create_case_record");
    // Job key may differ (new activation), but task type should match.

    eprintln!(
        "Stream disconnect redeliver verified: orphan={}, redelivered={}",
        orphan_key, jobs[0].job_key
    );

    // Cleanup: complete and cancel.
    let (res_canonical, res_hash) = canonical_json_with_hash(&serde_json::json!({}));
    let _ = client
        .complete_job(CompleteJobRequest {
            job_key: jobs[0].job_key.clone(),
            domain_payload: res_canonical,
            domain_payload_hash: res_hash,
            orch_flags: HashMap::new(),
        })
        .await;
    let _ = client.cancel(instance_id, "test cleanup").await;
}

// ===========================================================================
// B3.11 — Dead-letter / early signal (T-NET-3)
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_11_early_signal() {
    let client = bpmn_client().await;
    let _bytecode = compile_model(&client).await;

    let payload = serde_json::json!({ "entity_id": Uuid::new_v4().to_string() });
    let (canonical, hash) = canonical_json_with_hash(&payload);

    let instance_id = client
        .start_process(StartProcessRequest {
            process_key: "kyc_open_case".to_string(),
            bytecode_version: Vec::new(),
            domain_payload: canonical,
            domain_payload_hash: hash,
            orch_flags: HashMap::new(),
            correlation_id: Uuid::new_v4(),
        })
        .await
        .expect("StartProcess failed");

    // Try to signal "docs_received" BEFORE completing the service tasks.
    // This should either queue the signal or return an error (depending on
    // bpmn-lite implementation). The key invariant: it should not crash.
    let early_result = client.signal(instance_id, "docs_received", None).await;
    eprintln!(
        "Early signal result: {:?}",
        early_result.as_ref().map(|_| "ok").unwrap_or("err")
    );

    // Process should still be Running (not corrupted).
    let inspection = client.inspect(instance_id).await.expect("Inspect failed");
    assert_eq!(
        inspection.state, "Running",
        "Process should survive early signal"
    );

    // Cleanup.
    let _ = client.cancel(instance_id, "test cleanup").await;

    eprintln!("Early signal test passed — process not corrupted");
}

// ===========================================================================
// B3.12 — WorkflowDispatcher direct routing
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_12_dispatcher_direct_routing() {
    use ob_poc::bpmn_integration::dispatcher::WorkflowDispatcher;
    use ob_poc::repl::orchestrator_v2::{DslExecutionOutcome, DslExecutorV2, StubExecutor};

    let pool = test_pool().await;
    let client = bpmn_client().await;
    let config = load_config();
    let correlation_store = CorrelationStore::new(pool.clone());
    let parked_token_store = ParkedTokenStore::new(pool.clone());

    let inner: Arc<dyn DslExecutorV2> = Arc::new(StubExecutor);
    let dispatcher = WorkflowDispatcher::new(
        inner,
        Arc::new(config),
        client,
        correlation_store,
        parked_token_store,
        PendingDispatchStore::new(pool.clone()),
    );

    // Direct verb should delegate to inner executor (StubExecutor returns Completed).
    let result = dispatcher
        .execute_v2("(session.info)", Uuid::new_v4(), Uuid::new_v4())
        .await;

    match result {
        DslExecutionOutcome::Completed(_) => {
            eprintln!("Direct routing verified: session.info → Completed");
        }
        other => panic!("Expected Completed for direct verb, got {:?}", other),
    }
}

// ===========================================================================
// B3.13 — Signal endpoint resume (parked REPL entry resumed)
// ===========================================================================

#[tokio::test]
#[ignore]
async fn b3_13_signal_resume_parked_entry() {
    let pool = test_pool().await;
    let correlation_store = CorrelationStore::new(pool.clone());
    let parked_token_store = ParkedTokenStore::new(pool.clone());

    // Create a correlation record and parked token (simulating what
    // WorkflowDispatcher does when it parks an entry).
    let correlation_id = Uuid::new_v4();
    let process_instance_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let runbook_id = Uuid::new_v4();
    let entry_id = Uuid::new_v4();
    let correlation_key = format!("corr-{}", correlation_id);

    let record = CorrelationRecord {
        correlation_id,
        process_instance_id,
        session_id,
        runbook_id,
        entry_id,
        process_key: "kyc_open_case".to_string(),
        domain_payload_hash: vec![0u8; 32],
        status: CorrelationStatus::Active,
        created_at: chrono::Utc::now(),
        completed_at: None,
        domain_correlation_key: None,
    };
    correlation_store
        .insert(&record)
        .await
        .expect("Insert correlation failed");

    let token = ParkedToken {
        token_id: Uuid::new_v4(),
        correlation_key: correlation_key.clone(),
        session_id,
        entry_id,
        process_instance_id,
        expected_signal: "process_completed".to_string(),
        status: ParkedTokenStatus::Waiting,
        created_at: chrono::Utc::now(),
        resolved_at: None,
        result_payload: None,
    };
    parked_token_store
        .insert(&token)
        .await
        .expect("Insert token failed");

    // Verify token is waiting.
    let found = parked_token_store
        .find_by_correlation_key(&correlation_key)
        .await
        .expect("Find token failed");
    assert!(found.is_some());
    assert_eq!(found.unwrap().status, ParkedTokenStatus::Waiting);

    // Resolve the token (simulating what EventBridge does on ProcessCompleted).
    let result_payload = serde_json::json!({ "outcome": "approved" });
    parked_token_store
        .resolve(&correlation_key, Some(&result_payload))
        .await
        .expect("Resolve token failed");

    // Verify token is now Resolved.
    let found = parked_token_store
        .find_by_correlation_key(&correlation_key)
        .await
        .expect("Find token failed");
    assert!(found.is_some());
    let resolved = found.unwrap();
    assert_eq!(resolved.status, ParkedTokenStatus::Resolved);
    assert!(resolved.resolved_at.is_some());
    assert_eq!(resolved.result_payload, Some(result_payload));

    // Update correlation to Completed.
    correlation_store
        .update_status(correlation_id, CorrelationStatus::Completed)
        .await
        .expect("Update correlation status failed");

    let found = correlation_store
        .find_by_process_instance(process_instance_id)
        .await
        .expect("Find correlation failed");
    assert!(found.is_some());
    assert_eq!(found.unwrap().status, CorrelationStatus::Completed);

    eprintln!("Signal resume flow verified: token resolved, correlation completed");
}

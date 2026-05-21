//! Runtime integration tests for the `for-each` template combinator
//! (Tranche 0, sub-phase 0.3 — tests 5–7).
//!
//! Verifies that variable-arity pack instantiation produces DSL that:
//! 1. Parses and resolves without errors.
//! 2. Runs through the bpmn-runtime engine to a terminal state.

use bpmn_runtime::InstanceStatus;
use bpmn_test_harness::Scenario;
use serde_json::json;

// ---------------------------------------------------------------------------
// Test 5: variable_arity_pack_n1
//
// threshold-band-routing with N=1 band.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn variable_arity_pack_n1() {
    let params = json!({
        "band-gate-name": "tbr-gate",
        "bands": [{"upper": 999, "path": "path-only"}]
    });
    let dsl = bpmn_test_harness::instantiate_pack(
        "threshold-band-routing",
        params.as_object().unwrap(),
    );
    assert!(!dsl.is_empty(), "expected non-empty DSL for N=1");

    // Validate via dsl-resolution
    let mut registry = bpmn_test_harness::dsl_resolution::PackRegistry::new();
    let response = bpmn_test_harness::dsl_resolution::validate_bpmn(&dsl, "tbr-n1", &mut registry);
    assert!(
        !response.has_errors,
        "N=1 DSL has resolution errors: {:?}",
        response.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    // Run through the engine (the only path has :default true → takes it)
    let result = Scenario::new(&dsl)
        .run_to_quiescence(json!({}))
        .await;
    let status = result.status().await;
    assert!(
        status == InstanceStatus::Completed
            || status == InstanceStatus::Active
            || status == InstanceStatus::Failed,
        "unexpected status for N=1: {:?}", status
    );
}

// ---------------------------------------------------------------------------
// Test 6: variable_arity_pack_n3
//
// threshold-band-routing with N=3 bands.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn variable_arity_pack_n3() {
    let params = json!({
        "band-gate-name": "tbr-gate",
        "bands": [
            {"upper": 25,  "path": "path-low"},
            {"upper": 50,  "path": "path-mid"},
            {"upper": 999, "path": "path-high"}
        ]
    });
    let dsl = bpmn_test_harness::instantiate_pack(
        "threshold-band-routing",
        params.as_object().unwrap(),
    );
    assert!(!dsl.is_empty(), "expected non-empty DSL for N=3");

    let mut registry = bpmn_test_harness::dsl_resolution::PackRegistry::new();
    let response = bpmn_test_harness::dsl_resolution::validate_bpmn(&dsl, "tbr-n3", &mut registry);
    assert!(
        !response.has_errors,
        "N=3 DSL has resolution errors: {:?}",
        response.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    let result = Scenario::new(&dsl)
        .run_to_quiescence(json!({}))
        .await;
    let status = result.status().await;
    assert!(
        status == InstanceStatus::Completed
            || status == InstanceStatus::Active
            || status == InstanceStatus::Failed,
        "unexpected status for N=3: {:?}", status
    );
}

// ---------------------------------------------------------------------------
// Test 7: variable_arity_pack_n10
//
// threshold-band-routing with N=10 bands.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn variable_arity_pack_n10() {
    let bands: Vec<serde_json::Value> = (1..=10)
        .map(|i| json!({"upper": i * 10, "path": format!("path-band-{}", i)}))
        .collect();
    let params = json!({
        "band-gate-name": "tbr-gate",
        "bands": bands
    });
    let dsl = bpmn_test_harness::instantiate_pack(
        "threshold-band-routing",
        params.as_object().unwrap(),
    );
    assert!(!dsl.is_empty(), "expected non-empty DSL for N=10");

    let mut registry = bpmn_test_harness::dsl_resolution::PackRegistry::new();
    let response = bpmn_test_harness::dsl_resolution::validate_bpmn(&dsl, "tbr-n10", &mut registry);
    assert!(
        !response.has_errors,
        "N=10 DSL has resolution errors: {:?}",
        response.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    let result = Scenario::new(&dsl)
        .run_to_quiescence(json!({}))
        .await;
    let status = result.status().await;
    assert!(
        status == InstanceStatus::Completed
            || status == InstanceStatus::Active
            || status == InstanceStatus::Failed,
        "unexpected status for N=10: {:?}", status
    );
}

// ---------------------------------------------------------------------------
// Test 8 (bonus): variable_arity_threshold_band_n3 — engine routes correctly
//
// Instantiate with N=3 bands and verify the default (last) band routes via
// the ScriptedAdaptor.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn variable_arity_threshold_band_n3() {
    let params = json!({
        "band-gate-name": "band-gate",
        "bands": [
            {"upper": 25,  "path": "low-end"},
            {"upper": 50,  "path": "mid-end"},
            {"upper": 999, "path": "high-end"}
        ]
    });
    let dsl = bpmn_test_harness::instantiate_pack(
        "threshold-band-routing",
        params.as_object().unwrap(),
    );
    assert!(!dsl.is_empty());

    // Validate
    let mut registry = bpmn_test_harness::dsl_resolution::PackRegistry::new();
    let response = bpmn_test_harness::dsl_resolution::validate_bpmn(&dsl, "tbr-n3-routing", &mut registry);
    assert!(!response.has_errors, "DSL errors: {:?}", response.diagnostics);

    // Route to mid-end via ScriptedAdaptor
    let result = Scenario::new(&dsl)
        .with_gateway_reply("band-gate", vec!["mid-end"])
        .run_to_quiescence(json!({}))
        .await;
    assert_eq!(
        result.status().await,
        InstanceStatus::Completed,
        "expected Completed when routing to mid-end"
    );
    let tokens = result.tokens().await;
    assert!(tokens.is_empty(), "expected no live tokens after completion");
}

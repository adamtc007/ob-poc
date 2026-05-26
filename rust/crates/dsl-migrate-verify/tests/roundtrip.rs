//! Round-trip tests: each dsl-migrate corpus file should parse, validate,
//! lower, and start in the runtime.

use dsl_migrate::parse_bpmn_xml;
use dsl_migrate_verify::verify_dsl_source;

async fn roundtrip(bpmn_xml: &str, name: &str) {
    let process = parse_bpmn_xml(bpmn_xml).expect("bpmn parse failed");
    let result = dsl_migrate::emit(&process);
    let verify = verify_dsl_source(&result.dsl_source, name).await;
    assert!(
        verify.is_ok(),
        "round-trip failed for {name}: {:?}\n\nDSL:\n{}",
        verify.diagnostics,
        result.dsl_source
    );
}

#[tokio::test]
async fn linear_sequence_roundtrips() {
    roundtrip(include_str!("../../dsl-migrate/tests/corpus/linear_sequence.bpmn"), "linear-sequence").await;
}

#[tokio::test]
async fn exclusive_gateway_roundtrips() {
    roundtrip(include_str!("../../dsl-migrate/tests/corpus/exclusive_gateway.bpmn"), "exclusive-gateway").await;
}

#[tokio::test]
async fn parallel_fork_join_roundtrips() {
    roundtrip(include_str!("../../dsl-migrate/tests/corpus/parallel_fork_join.bpmn"), "parallel-fork-join").await;
}

#[tokio::test]
async fn boundary_events_roundtrip() {
    roundtrip(include_str!("../../dsl-migrate/tests/corpus/boundary_events.bpmn"), "boundary-events").await;
}

#[tokio::test]
async fn feel_expressions_roundtrip() {
    // Conditions are now normalised (not TODO) — round-trip should succeed.
    roundtrip(include_str!("../../dsl-migrate/tests/corpus/feel_expressions.bpmn"), "feel-expressions").await;
}

#[tokio::test]
async fn feel_conditions_complex_roundtrip() {
    // Out-of-scope FEEL produces HUMAN-RESOLVE comments but DSL is still valid.
    roundtrip(include_str!("../../dsl-migrate/tests/corpus/feel_conditions_complex.bpmn"), "feel-complex").await;
}

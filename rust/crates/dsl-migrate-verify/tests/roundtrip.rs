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

#[tokio::test]
async fn user_task_with_form_roundtrips() {
    // Tasks with formKey emit :verb dsl.form — DSL should still parse and validate.
    roundtrip(include_str!("../../dsl-migrate/tests/corpus/user_task_with_form.bpmn"), "user-task-form").await;
}

// ── Negative tests — bad Camunda source rejected at verify stage ─────────────

#[tokio::test]
async fn gateway_with_no_outgoing_flows_fails_verification() {
    let bpmn = r#"<?xml version="1.0" encoding="UTF-8"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
    id="bad-gw-def" targetNamespace="http://example.com">
  <bpmn:process id="bad-gw" name="Bad Gateway" isExecutable="true">
    <bpmn:startEvent id="start-1" name="Start"/>
    <bpmn:exclusiveGateway id="gw-dead" name="Dead Gateway"/>
    <bpmn:sequenceFlow id="sf1" sourceRef="start-1" targetRef="gw-dead"/>
    <!-- no outgoing flows from gateway — assembler should reject this -->
  </bpmn:process>
</bpmn:definitions>"#;

    let process = dsl_migrate::parse_bpmn_xml(bpmn).expect("bpmn parse");
    let result = dsl_migrate::emit(&process);
    let verify = verify_dsl_source(&result.dsl_source, "bad-gw").await;
    assert!(
        !verify.is_ok(),
        "gateway with no outgoing flows should fail verification; diagnostics: {:?}\nDSL:\n{}",
        verify.diagnostics,
        result.dsl_source
    );
    assert!(
        !verify.diagnostics.is_empty(),
        "expected at least one diagnostic for dead-end gateway"
    );
}

#[tokio::test]
async fn process_with_no_end_event_fails_verification() {
    let bpmn = r#"<?xml version="1.0" encoding="UTF-8"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
    id="no-end-def" targetNamespace="http://example.com">
  <bpmn:process id="no-end" name="No End" isExecutable="true">
    <bpmn:startEvent id="start-1" name="Start"/>
    <bpmn:serviceTask id="task-1" name="Do Work"/>
    <bpmn:sequenceFlow id="sf1" sourceRef="start-1" targetRef="task-1"/>
    <!-- no end event — assembler should reject: no terminal node -->
  </bpmn:process>
</bpmn:definitions>"#;

    let process = dsl_migrate::parse_bpmn_xml(bpmn).expect("bpmn parse");
    let result = dsl_migrate::emit(&process);
    let verify = verify_dsl_source(&result.dsl_source, "no-end").await;
    assert!(
        !verify.is_ok(),
        "process with no end event should fail verification; diagnostics: {:?}\nDSL:\n{}",
        verify.diagnostics,
        result.dsl_source
    );
}

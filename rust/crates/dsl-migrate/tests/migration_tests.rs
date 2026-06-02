//! Integration tests for the dsl-migrate crate.

#[test]
fn linear_sequence_migrates_cleanly() {
    let xml = include_str!("corpus/linear_sequence.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);

    // No human-resolve items expected: entity-verify + sanctions-check both resolve
    assert_eq!(
        result.coverage.human_resolve,
        0,
        "no human-resolve expected; got: {:?}",
        result
            .coverage
            .elements
            .iter()
            .filter(|e| e.status == dsl_migrate::MigrationStatus::HumanResolve)
            .collect::<Vec<_>>()
    );
    assert_eq!(result.coverage.rejected, 0, "no rejected expected");

    // DSL must reference start and end event kinds
    assert!(
        result.dsl_source.contains("start"),
        "DSL should contain start node"
    );
    assert!(
        result.dsl_source.contains("end"),
        "DSL should contain end node"
    );

    // Print for visual inspection
    eprintln!("=== linear_sequence DSL ===\n{}", result.dsl_source);
    eprintln!("Coverage: {}", result.coverage.summary());
}

#[test]
fn exclusive_gateway_migrates_cleanly() {
    let xml = include_str!("corpus/exclusive_gateway.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);

    assert!(
        result.dsl_source.contains("exclusive"),
        "DSL should contain exclusive gateway"
    );
    assert!(result.coverage.clean > 0, "should have clean elements");
    assert_eq!(result.coverage.rejected, 0, "no rejected expected");
}

#[test]
fn parallel_fork_join_migrates() {
    let xml = include_str!("corpus/parallel_fork_join.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);

    assert!(
        result.dsl_source.contains("parallel"),
        "DSL should contain parallel gateway"
    );
    // Should have at least start + 2 tasks + 2 gateways + end = 6 elements
    assert!(
        result.coverage.total >= 6,
        "expected at least 6 elements, got {}",
        result.coverage.total
    );
}

#[test]
fn boundary_events_produce_attachment_atoms() {
    let xml = include_str!("corpus/boundary_events.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);

    assert!(
        result.dsl_source.contains("boundary-attachment"),
        "boundary events should produce boundary-attachment atoms; DSL:\n{}",
        result.dsl_source
    );
    // Two boundary events: error + timer
    let count = result.dsl_source.matches("boundary-attachment").count();
    assert!(
        count >= 2,
        "expected at least 2 boundary-attachment atoms, got {}",
        count
    );
}

#[test]
fn feel_expressions_normalise_cleanly() {
    let xml = include_str!("corpus/feel_expressions.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);

    // ${score >= 700} and ${score < 700} are within the supported subset — no HUMAN-RESOLVE
    assert_eq!(
        result.coverage.human_resolve,
        0,
        "no human-resolve expected; got: {:?}",
        result
            .coverage
            .elements
            .iter()
            .filter(|e| e.status == dsl_migrate::MigrationStatus::HumanResolve)
            .collect::<Vec<_>>()
    );
    // Conditions emitted verbatim (stripped of Juel wrappers)
    assert!(
        result.dsl_source.contains(":condition \"score >= 700\""),
        "expected normalised condition in DSL:\n{}",
        result.dsl_source
    );
    assert!(
        result.dsl_source.contains(":condition \"score < 700\""),
        "expected normalised condition in DSL:\n{}",
        result.dsl_source
    );
}

#[test]
fn complex_feel_out_of_scope_becomes_human_resolve() {
    let xml = include_str!("corpus/feel_conditions_complex.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);

    // Simple conditions should still resolve cleanly
    assert!(
        result.dsl_source.contains(":condition \"amount > 1000\""),
        "simple condition should be clean:\n{}",
        result.dsl_source
    );
    // Dot-access should produce HUMAN-RESOLVE
    assert!(
        result.dsl_source.contains("HUMAN-RESOLVE"),
        "dot-access condition should produce HUMAN-RESOLVE:\n{}",
        result.dsl_source
    );
    assert!(
        result.coverage.human_resolve > 0,
        "expected at least one human-resolve for out-of-scope FEEL"
    );
}

#[test]
fn user_task_form_key_mapping() {
    let xml = include_str!("corpus/user_task_with_form.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);
    let dsl = &result.dsl_source;

    // Plain key passthrough
    assert!(
        dsl.contains(r#":verb dsl.form :form-ref "kyc.review-summary""#),
        "plain key should pass through:\n{}",
        dsl
    );
    // embedded: prefix stripped
    assert!(
        dsl.contains(r#":verb dsl.form :form-ref "embedded/onboarding-review""#),
        "embedded prefix should be stripped:\n{}",
        dsl
    );
    // deployment: prefix stripped
    assert!(
        dsl.contains(r#":verb dsl.form :form-ref "deployment/checklist.json""#),
        "deployment prefix should be stripped:\n{}",
        dsl
    );
    // classpath: → HUMAN-RESOLVE
    assert!(
        dsl.contains("HUMAN-RESOLVE"),
        "classpath formKey should produce HUMAN-RESOLVE:\n{}",
        dsl
    );
    // No formKey → plain user-task, no dsl.form
    assert!(
        dsl.contains("(node task-no-form :kind user-task)"),
        "missing formKey should emit plain user-task:\n{}",
        dsl
    );
}

#[test]
fn complex_gateway_is_rejected() {
    let xml = r#"<?xml version="1.0"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL" id="d1">
  <bpmn:process id="p1">
    <bpmn:complexGateway id="cg1" name="Complex"/>
  </bpmn:process>
</bpmn:definitions>"#;

    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);
    assert_eq!(
        result.coverage.rejected,
        1,
        "complex gateway should be rejected; got coverage: {:?}",
        result.coverage.summary()
    );
}

#[test]
fn coverage_report_sums_correctly() {
    let xml = include_str!("corpus/feel_expressions.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);
    let r = &result.coverage;
    // Skipped boundary events are counted but shouldn't break totals
    assert_eq!(
        r.clean + r.human_resolve + r.rejected + r.skipped,
        r.total,
        "coverage totals must sum to total; breakdown: clean={} hr={} rej={} skipped={} total={}",
        r.clean,
        r.human_resolve,
        r.rejected,
        r.skipped,
        r.total,
    );
}

#[test]
fn process_name_extracted() {
    let xml = include_str!("corpus/linear_sequence.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);
    assert_eq!(result.process_name, "Linear Sequence");
}

#[test]
fn migration_source_atom_present() {
    let xml = include_str!("corpus/linear_sequence.bpmn");
    let process = dsl_migrate::parse_bpmn_xml(xml).unwrap();
    let result = dsl_migrate::emit(&process);
    assert!(
        result.dsl_source.contains("migration-source"),
        "DSL should contain migration-source provenance atom"
    );
}

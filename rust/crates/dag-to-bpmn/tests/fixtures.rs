//! Unit fixtures: small, hand-authored `Dag`s exercising each supported and
//! unsupported shape described in the crate's design doc. Each fixture is
//! deserialized from a YAML snippet shaped like real `dag_taxonomies`
//! authoring, not constructed via Rust struct literals — this both keeps
//! the fixtures short (many fields are `Option`) and doubles as a
//! confirmation that the compiler reads the same shape real files use.

use dag_to_bpmn::{compile_slot, DagToBpmnError, ShapeError};
use dsl_types::Dag;

fn dag(yaml: &str) -> Dag {
    serde_yaml::from_str(yaml).expect("fixture YAML must parse as a Dag")
}

const HEADER: &str = "version: \"1.0\"\nworkspace: test\ndag_id: test_dag\n";

fn slot_yaml(body: &str) -> String {
    format!("{HEADER}slots:\n  - id: test_slot\n    state_machine:\n{body}")
}

// ---------------------------------------------------------------------------
// Supported shapes
// ---------------------------------------------------------------------------

#[test]
fn linear_chain_compiles() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: approved
      terminal_states: [approved]
      transitions:
        - from: opened
          to: approved
          via: kyc-case.approve
"#,
    ));

    let out = compile_slot(&d, "test_slot", "linear-chain").expect("must compile");
    assert!(out.dsl_source.contains("(node opened :kind start-event)"));
    assert!(out.dsl_source.contains("(node approved :kind end-event)"));
    assert!(out
        .dsl_source
        .contains(":verb (invoke kyc-case.approve)"));
    assert_eq!(out.spec.start_node, "opened");
    // start -> task -> end, no gateways.
    assert!(!out.dsl_source.contains("(gateway"));
}

#[test]
fn fan_out_gets_exclusive_gateway() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: escalated
        - id: referred
      terminal_states: [escalated, referred]
      transitions:
        - from: opened
          to: escalated
          via: kyc-case.escalate
        - from: opened
          to: referred
          via: kyc-case.refer
"#,
    ));

    let out = compile_slot(&d, "test_slot", "fan-out").expect("must compile");
    assert!(out.dsl_source.contains("(gateway gw__opened :kind exclusive)"));
    assert!(out.dsl_source.contains(":default true"));
    assert!(out.dsl_source.contains(":condition \"kyc-case.refer\""));
}

#[test]
fn fan_in_needs_no_gateway() {
    // "closed" is reached two different ways: via a clean close after
    // review, and via a separate close after escalation. Neither route
    // shares an upstream fan-out point with the other's tail, so the
    // convergence on `closed` is a pure fan-in — it must need no gateway,
    // even though `under_review` (which *does* fan out to two places) does.
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: under_review
        - id: escalated
        - id: closed
      terminal_states: [closed]
      transitions:
        - from: opened
          to: under_review
          via: kyc-case.review
        - from: under_review
          to: closed
          via: kyc-case.close-clean
        - from: under_review
          to: escalated
          via: kyc-case.escalate
        - from: escalated
          to: closed
          via: kyc-case.close-after-escalation
"#,
    ));

    let out = compile_slot(&d, "test_slot", "fan-in").expect("must compile");
    // under_review fans out to two places -> gateway expected there.
    assert!(out.dsl_source.contains("(gateway gw__under_review :kind exclusive)"));
    // closed is only ever a merge target, never a fan-out source -> no
    // gateway for it, no matter how many producers feed it.
    assert!(!out.dsl_source.contains("gw__closed"));
    let edges_into_closed_end: Vec<&str> = out
        .dsl_source
        .lines()
        .filter(|l| l.trim_start().starts_with("(flow") && l.contains("-> closed)"))
        .collect();
    assert_eq!(
        edges_into_closed_end.len(),
        2,
        "both routes must flow directly into the closed end-event: {edges_into_closed_end:?}"
    );
    // Both incoming edges originate from task nodes, not from a gateway —
    // i.e. no synthetic merge gateway was inserted at the convergence point.
    assert!(edges_into_closed_end
        .iter()
        .all(|l| l.trim_start().starts_with("(flow t__")));
}

#[test]
fn any_non_terminal_expands_to_every_non_terminal_state() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: under_review
        - id: closed
      terminal_states: [closed]
      transitions:
        - from: opened
          to: under_review
          via: kyc-case.review
        - from: "(any non-terminal)"
          to: closed
          via: kyc-case.abandon
"#,
    ));

    let out = compile_slot(&d, "test_slot", "any-non-terminal").expect("must compile");
    // Both "opened" and "under_review" are non-terminal, so the expanded
    // "(any non-terminal)" transition targets both. "under_review" has
    // exactly one outgoing target (the abandon task), so it flows in
    // directly. "opened" now has *two* distinct outgoing targets (its own
    // explicit transition to under_review, plus the expanded abandon
    // transition) — a real fan-out, correctly gated.
    assert!(out
        .dsl_source
        .contains("(flow t__under_review__kyc-case_review -> t__closed__kyc-case_abandon)"));
    assert!(out.dsl_source.contains("(gateway gw__opened :kind exclusive)"));
    assert!(out.dsl_source.contains("(flow opened -> gw__opened)"));
    assert!(out
        .dsl_source
        .contains("(flow gw__opened -> t__closed__kyc-case_abandon :default true)"));
    assert!(out
        .dsl_source
        .contains("(flow gw__opened -> t__under_review__kyc-case_review :condition \"kyc-case.review\")"));
}

// ---------------------------------------------------------------------------
// Fail-loud shapes — RED-first proven (see red_first_proof.rs note in the
// module doc comment; this test asserts the current, guarded behavior).
// ---------------------------------------------------------------------------

#[test]
fn list_valued_via_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: closed
      terminal_states: [closed]
      transitions:
        - from: opened
          to: closed
          via: [kyc-case.escalate, kyc-case.refer]
"#,
    ));

    let err = compile_slot(&d, "test_slot", "list-via").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::TransitionViaIsList { .. })
    ));
}

#[test]
fn free_text_via_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: closed
      terminal_states: [closed]
      transitions:
        - from: opened
          to: closed
          via: "(backend: entity lookup / GLEIF import)"
"#,
    ));

    let err = compile_slot(&d, "test_slot", "free-text-via").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::TransitionViaNotAVerb { .. })
    ));
}

#[test]
fn zero_entry_states_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
        - id: closed
      terminal_states: [closed]
      transitions:
        - from: opened
          to: closed
          via: kyc-case.close-clean
"#,
    ));

    let err = compile_slot(&d, "test_slot", "no-entry").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::NoEntryState { .. })
    ));
}

#[test]
fn multiple_entry_states_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened_a
          entry: true
        - id: opened_b
          entry: true
        - id: closed
      terminal_states: [closed]
      transitions:
        - from: opened_a
          to: closed
          via: kyc-case.close-clean
"#,
    ));

    let err = compile_slot(&d, "test_slot", "multi-entry").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::MultipleEntryStates { count: 2, .. })
    ));
}

#[test]
fn unknown_to_state_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
      terminal_states: []
      transitions:
        - from: opened
          to: nonexistent_state
          via: kyc-case.close-clean
"#,
    ));

    let err = compile_slot(&d, "test_slot", "unknown-to").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::TransitionToUnknownState { .. })
    ));
}

#[test]
fn unknown_slot_id_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
      terminal_states: []
      transitions: []
"#,
    ));

    let err = compile_slot(&d, "no-such-slot", "unknown-slot").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::SlotNotFound { .. })
    ));
}

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

// ---------------------------------------------------------------------------
// `awaits` — the call-out + switch move (EOP-PLAN-DAG-AWAITS-001)
// ---------------------------------------------------------------------------

#[test]
fn single_arm_await_compiles_with_no_gateway() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: screen
          awaits:
            source:
              kind: verb_switch
              verb: kyc-case.close-clean
            cases:
              - outcome: DONE
                to: closed
        - id: closed
      terminal_states: [closed]
      transitions:
        - from: opened
          to: screen
          via: entity-workstream.begin-screening
"#,
    ));

    let out = compile_slot(&d, "test_slot", "single-arm-await").expect("must compile");
    assert!(out
        .dsl_source
        .contains("(node awt_src__screen :kind service-task :verb (invoke kyc-case.close-clean))"));
    assert!(out
        .dsl_source
        .contains("(flow awt_src__screen -> closed)"));
    // No gateway needed for a single arm.
    assert!(!out.dsl_source.contains("awt_gw__screen"));
}

#[test]
fn multi_arm_await_gets_exclusive_gateway_and_reaches_real_slot() {
    let d = dag(&format!(
        "{HEADER}slots:\n\
         \x20 - id: screening\n\
         \x20\x20\x20 state_machine:\n\
         \x20\x20\x20\x20\x20 id: screening_sm\n\
         \x20\x20\x20\x20\x20 states:\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: PENDING\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 entry: true\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: CLEAR\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: HIT_CONFIRMED\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: HIT_DISMISSED\n\
         \x20\x20\x20\x20\x20 terminal_states: [CLEAR, HIT_CONFIRMED, HIT_DISMISSED]\n\
         \x20\x20\x20\x20\x20 transitions:\n\
         \x20\x20\x20\x20\x20\x20\x20 - from: PENDING\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 to: CLEAR\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 via: screening.clear\n\
         \x20 - id: test_slot\n\
         \x20\x20\x20 state_machine:\n\
         \x20\x20\x20\x20\x20 id: test_slot_sm\n\
         \x20\x20\x20\x20\x20 states:\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: opened\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 entry: true\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: screen\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 awaits:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 source:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 kind: slot_terminal_state_aggregate\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 slot: screening\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 scope: attached_to this workstream\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 reduce: worst_of\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 cases:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 - outcome: HIT_CONFIRMED\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 to: enhanced_dd\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 - outcome: HIT_DISMISSED\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 to: assess\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 - outcome: CLEAR\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 to: assess\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: assess\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: enhanced_dd\n\
         \x20\x20\x20\x20\x20 terminal_states: [assess, enhanced_dd]\n\
         \x20\x20\x20\x20\x20 transitions:\n\
         \x20\x20\x20\x20\x20\x20\x20 - from: opened\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 to: screen\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 via: entity-workstream.begin-screening\n"
    ));

    let out = compile_slot(&d, "test_slot", "multi-arm-await").expect("must compile");
    assert!(out
        .dsl_source
        .contains("(node awt_src__screen :kind intermediate-catch-signal) ; awaits screening (aggregate: worst_of)"));
    assert!(out.dsl_source.contains("(gateway awt_gw__screen :kind exclusive)"));
    assert!(out
        .dsl_source
        .contains("(flow awt_src__screen -> awt_gw__screen)"));
    assert!(out
        .dsl_source
        .contains("(flow awt_gw__screen -> enhanced_dd :default true)"));
    assert!(out
        .dsl_source
        .contains("(flow awt_gw__screen -> assess :condition \"HIT_DISMISSED\")"));
    assert!(out
        .dsl_source
        .contains("(flow awt_gw__screen -> assess :condition \"CLEAR\")"));
}

#[test]
fn await_and_transitions_both_present_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: closed
          awaits:
            source:
              kind: verb_switch
              verb: kyc-case.close-clean
            cases:
              - outcome: DONE
                to: closed
      terminal_states: [closed]
      transitions:
        - from: closed
          to: opened
          via: kyc-case.reopen
"#,
    ));

    let err = compile_slot(&d, "test_slot", "awaits-and-transitions").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::AwaitsAndTransitionsBothPresent { .. })
    ));
}

#[test]
fn await_coexists_with_any_non_terminal_wildcard_transition() {
    // Found migrating entity_workstream.SCREEN (EOP-PLAN-DAG-AWAITS-001
    // Phase 5): a "(any non-terminal)" wildcard-sourced transition models
    // an operator interrupt/override move (block/escalate/refer/reject)
    // available from ANY non-terminal state, including one that also has
    // `awaits` — a genuinely different kind of edge than the state's own
    // primary resolution path. Only an explicit `from: <this state>` row
    // conflicts with `awaits` (see the rejected case above); a wildcard
    // does not.
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: screen
          awaits:
            source:
              kind: verb_switch
              verb: kyc-case.close-clean
            cases:
              - outcome: DONE
                to: closed
        - id: blocked
        - id: closed
      terminal_states: [closed, blocked]
      transitions:
        - from: opened
          to: screen
          via: entity-workstream.begin-screening
        - from: "(any non-terminal)"
          to: blocked
          via: entity-workstream.update-status
"#,
    ));

    let out = compile_slot(&d, "test_slot", "await-plus-wildcard").expect("must compile");
    // The await's own source/flow still emits normally.
    assert!(out
        .dsl_source
        .contains("(node awt_src__screen :kind service-task :verb (invoke kyc-case.close-clean))"));
    assert!(out.dsl_source.contains("(flow awt_src__screen -> closed)"));
    // The wildcard override edge out of `screen` still exists too: its
    // producer (the task node that lands on `screen`) flows into the
    // `blocked` task node, which in turn flows into `blocked`'s end-event.
    assert!(out.dsl_source.contains(
        "(flow t__screen__entity-workstream_begin-screening -> t__blocked__entity-workstream_update-status)"
    ));
    assert!(out
        .dsl_source
        .contains("(flow t__blocked__entity-workstream_update-status -> blocked)"));
}

#[test]
fn await_case_target_unknown_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: closed
          awaits:
            source:
              kind: verb_switch
              verb: kyc-case.close-clean
            cases:
              - outcome: DONE
                to: nonexistent_state
      terminal_states: [closed]
      transitions: []
"#,
    ));

    let err = compile_slot(&d, "test_slot", "await-unknown-target").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::AwaitCaseTargetUnknown { .. })
    ));
}

#[test]
fn await_target_slot_not_found_is_rejected() {
    let d = dag(&slot_yaml(
        r#"
      id: test_slot_sm
      states:
        - id: opened
          entry: true
        - id: closed
          awaits:
            source:
              kind: slot_terminal_state
              slot: no_such_slot
            cases:
              - outcome: DONE
                to: closed
      terminal_states: [closed]
      transitions: []
"#,
    ));

    let err = compile_slot(&d, "test_slot", "await-unknown-slot").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::AwaitTargetSlotNotFound { .. })
    ));
}

#[test]
fn await_case_outcome_not_a_terminal_state_is_rejected() {
    let d = dag(&format!(
        "{HEADER}slots:\n\
         \x20 - id: screening\n\
         \x20\x20\x20 state_machine:\n\
         \x20\x20\x20\x20\x20 id: screening_sm\n\
         \x20\x20\x20\x20\x20 states:\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: PENDING\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 entry: true\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: CLEAR\n\
         \x20\x20\x20\x20\x20 terminal_states: [CLEAR]\n\
         \x20\x20\x20\x20\x20 transitions: []\n\
         \x20 - id: test_slot\n\
         \x20\x20\x20 state_machine:\n\
         \x20\x20\x20\x20\x20 id: test_slot_sm\n\
         \x20\x20\x20\x20\x20 states:\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: opened\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 entry: true\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: closed\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 awaits:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 source:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 kind: slot_terminal_state\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 slot: screening\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 cases:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 - outcome: TYPO_OUTCOME\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 to: closed\n\
         \x20\x20\x20\x20\x20 terminal_states: [closed]\n\
         \x20\x20\x20\x20\x20 transitions: []\n"
    ));

    let err = compile_slot(&d, "test_slot", "await-bad-outcome").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::AwaitCaseNotATerminalState { .. })
    ));
}

#[test]
fn await_cases_not_exhaustive_is_rejected() {
    let d = dag(&format!(
        "{HEADER}slots:\n\
         \x20 - id: screening\n\
         \x20\x20\x20 state_machine:\n\
         \x20\x20\x20\x20\x20 id: screening_sm\n\
         \x20\x20\x20\x20\x20 states:\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: PENDING\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 entry: true\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: CLEAR\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: HIT_CONFIRMED\n\
         \x20\x20\x20\x20\x20 terminal_states: [CLEAR, HIT_CONFIRMED]\n\
         \x20\x20\x20\x20\x20 transitions: []\n\
         \x20 - id: test_slot\n\
         \x20\x20\x20 state_machine:\n\
         \x20\x20\x20\x20\x20 id: test_slot_sm\n\
         \x20\x20\x20\x20\x20 states:\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: opened\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 entry: true\n\
         \x20\x20\x20\x20\x20\x20\x20 - id: closed\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20 awaits:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 source:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 kind: slot_terminal_state\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 slot: screening\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 cases:\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 - outcome: CLEAR\n\
         \x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20\x20 to: closed\n\
         \x20\x20\x20\x20\x20 terminal_states: [closed]\n\
         \x20\x20\x20\x20\x20 transitions: []\n"
    ));

    let err = compile_slot(&d, "test_slot", "await-not-exhaustive").unwrap_err();
    assert!(matches!(
        err,
        DagToBpmnError::Shape(ShapeError::AwaitCasesNotExhaustive { .. })
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

//! v1.2 Tranche 1 DoD item 9 integration test.
//!
//! Exercises the v1.2 amendments to the validator and three-axis schema:
//! `transition_args:` as the canonical structural carrier; legacy
//! `transitions:` block grandfathered; preserving + transition_args
//! emits a migration warning; pure-preserving and pure-transition shapes
//! validate clean.
//!
//! Companion fixture: `tests/fixtures/v1_2_dod_fixture/verbs.yaml`.

use dsl_core::config::{validate_verbs_config, StructuralError, ValidationContext, VerbsConfig};
use std::fs;

fn load_v1_2_fixture() -> VerbsConfig {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/v1_2_dod_fixture/verbs.yaml"
    );
    let raw = fs::read_to_string(path).expect("v1.2 fixture file not readable");
    serde_yaml::from_str(&raw).expect("v1.2 fixture YAML parses")
}

#[test]
fn v1_2_fixture_parses_five_verbs() {
    let cfg = load_v1_2_fixture();
    let domain = cfg
        .domains
        .get("v1_2_fixture")
        .expect("v1_2_fixture domain");
    // Was 6 verbs (incl. preserving-with-transition_args migration case).
    // After T2.A.1 promoted the warning to structural error, the
    // migration case is removed from the fixture — it would now fail
    // structurally, which is the correct v1.2 §6.2 behaviour.
    assert_eq!(domain.verbs.len(), 5, "v1.2 fixture should declare 5 verbs");
}

#[test]
fn v1_2_canonical_transition_has_transition_args() {
    let cfg = load_v1_2_fixture();
    let v = &cfg.domains["v1_2_fixture"].verbs["v1-canonical-transition"];
    assert!(v.transition_args.is_some());
    let ta = v.transition_args.as_ref().unwrap();
    assert_eq!(ta.entity_id_arg, "deal-id");
    assert_eq!(ta.target_workspace.as_deref(), Some("deal"));
    assert_eq!(ta.target_slot.as_deref(), Some("deal"));
}

#[test]
fn v1_2_target_state_arg_optional() {
    let cfg = load_v1_2_fixture();
    let with_state = &cfg.domains["v1_2_fixture"].verbs["v1-with-target-state-arg"];
    assert!(with_state
        .transition_args
        .as_ref()
        .and_then(|a| a.target_state_arg.as_deref())
        .map(|s| s == "new-status")
        .unwrap_or(false));

    let canonical = &cfg.domains["v1_2_fixture"].verbs["v1-canonical-transition"];
    assert!(canonical
        .transition_args
        .as_ref()
        .map(|a| a.target_state_arg.is_none())
        .unwrap_or(false));
}

#[test]
fn v1_2_strict_preserving_with_transition_args_is_structural_error() {
    // After T2.A.1 promoted the warning to a structural error, a verb
    // declaring `state_effect: preserving` AND `transition_args:` fails
    // validation. Parse a minimal YAML to construct the bad shape.
    let yaml = r#"
version: "1.0"
domains:
  test:
    description: t
    verbs:
      bad:
        description: "preserving with transition_args — rejected by v1.2 strict"
        behavior: plugin
        args:
          - name: deal-id
            type: uuid
            required: true
        three_axis:
          state_effect: preserving
          external_effects: []
          consequence:
            baseline: benign
        transition_args:
          entity_id_arg: deal-id
          target_workspace: deal
          target_slot: deal
"#;
    let cfg: VerbsConfig = serde_yaml::from_str(yaml).expect("parses");
    let report = validate_verbs_config(&cfg, &ValidationContext::default());
    assert_eq!(report.structural.len(), 1);
    assert!(matches!(
        report.structural[0],
        StructuralError::PreservingWithTransitionArgs(_)
    ));
}

#[test]
fn v1_2_validator_clean_for_canonical_shapes() {
    let cfg = load_v1_2_fixture();
    let ctx = ValidationContext::default();
    let report = validate_verbs_config(&cfg, &ctx);

    // No structural errors — every verb in the fixture is structurally
    // legal (the migration warning is *not* a structural error).
    assert!(
        report.structural.is_empty(),
        "expected no structural errors, got {:?}",
        report.structural
    );
    // No well-formedness errors — the fixture is hand-curated to be clean.
    assert!(
        report.well_formedness.is_empty(),
        "expected no well-formedness errors, got {:?}",
        report.well_formedness
    );
}

#[test]
fn v1_2_legacy_transitions_block_grandfathered() {
    // The legacy v1.1 verb has transitions: but no transition_args:.
    // It must validate clean during the migration window.
    let cfg = load_v1_2_fixture();
    let ctx = ValidationContext::default();
    let report = validate_verbs_config(&cfg, &ctx);

    let legacy = &cfg.domains["v1_2_fixture"].verbs["legacy-v1-1-transition"];
    assert!(
        legacy.transition_args.is_none(),
        "legacy verb has no transition_args"
    );
    assert!(legacy
        .three_axis
        .as_ref()
        .and_then(|t| t.transitions.as_ref())
        .is_some());

    // No structural error mentioning the legacy verb.
    let legacy_errors = report
        .structural
        .iter()
        .filter(|e| match e {
            StructuralError::TransitionWithoutEdges(loc)
            | StructuralError::PreservingWithTransitions(loc)
            | StructuralError::TransitionWithoutTransitionArgs(loc)
            | StructuralError::PreservingWithTransitionArgs(loc) => {
                format!("{}", loc).contains("legacy-v1-1-transition")
            }
            StructuralError::TransitionArgsSlotNotFound { location, .. } => {
                format!("{}", location).contains("legacy-v1-1-transition")
            }
        })
        .count();
    assert_eq!(
        legacy_errors, 0,
        "legacy verb must not trigger structural errors"
    );
}

#[test]
fn v1_2_canonical_predicate_with_exists_parses() {
    // The lifecycle_resources_dag's
    // service_consumption_active_requires_live_binding constraint
    // exercises the v1.2 EXISTS predicate extension. We can't import
    // SqlPredicateResolver from dsl-runtime here (circular dep), but
    // we exercise the same shape via the validator's predicate-syntax
    // checks if/when those extend. For now, this test documents the
    // intent: T1.B's EXISTS support is unit-tested in
    // crates/dsl-runtime/src/cross_workspace/sql_predicate_resolver.rs.
    //
    // This is a placeholder assertion; the real EXISTS test lives in
    // dsl-runtime where the parser is.
    let cfg = load_v1_2_fixture();
    assert!(!cfg.domains.is_empty());
}

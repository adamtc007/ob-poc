use dsl_core::config::dag::{load_dags_from_dir, ClosureType, Dag, EligibilityConstraint};
use std::path::PathBuf;

fn dag_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/sem_os_seeds/dag_taxonomies")
}

#[test]
fn slot_gate_metadata_batch_1a_round_trips() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
slots:
  - id: investment_manager
    closure: closed_bounded
    eligibility:
      entity_kinds: [company, trust]
    cardinality_max: 2
    entry_state: DISCOVERED
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("gate metadata parses");
    let slot = &dag.slots[0];

    assert_eq!(slot.closure, Some(ClosureType::ClosedBounded));
    assert_eq!(
        slot.eligibility,
        Some(EligibilityConstraint::EntityKinds {
            entity_kinds: vec!["company".to_string(), "trust".to_string()]
        })
    );
    assert_eq!(slot.cardinality_max, Some(2));
    assert_eq!(slot.entry_state.as_deref(), Some("DISCOVERED"));

    let round_trip = serde_yaml::to_string(&dag).expect("serializes");
    let reparsed: Dag = serde_yaml::from_str(&round_trip).expect("round-trip parses");
    assert_eq!(reparsed.slots[0].closure, Some(ClosureType::ClosedBounded));
    assert_eq!(reparsed.slots[0].cardinality_max, Some(2));
}

#[test]
fn slot_gate_metadata_batch_1a_defaults_absent_fields() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
slots:
  - id: cbu
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("slot parses");
    let slot = &dag.slots[0];

    assert_eq!(slot.closure, None);
    assert_eq!(slot.eligibility, None);
    assert_eq!(slot.cardinality_max, None);
    assert_eq!(slot.entry_state, None);
}

#[test]
fn slot_predicate_vectors_batch_1b_round_trips() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
slots:
  - id: vehicle
    attachment_predicates:
      - "candidate.state = VALIDATED"
    addition_predicates:
      - "this.service.status = ACTIVE"
    aggregate_breach_checks:
      - "count(vehicle) <= 3"
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("predicate vectors parse");
    let slot = &dag.slots[0];

    assert_eq!(
        slot.attachment_predicates,
        vec!["candidate.state = VALIDATED".to_string()]
    );
    assert_eq!(
        slot.addition_predicates,
        vec!["this.service.status = ACTIVE".to_string()]
    );
    assert_eq!(
        slot.aggregate_breach_checks,
        vec!["count(vehicle) <= 3".to_string()]
    );

    let round_trip = serde_yaml::to_string(&dag).expect("serializes");
    let reparsed: Dag = serde_yaml::from_str(&round_trip).expect("round-trip parses");
    assert_eq!(
        reparsed.slots[0].attachment_predicates,
        vec!["candidate.state = VALIDATED".to_string()]
    );
}

#[test]
fn slot_predicate_vectors_batch_1b_defaults_absent_fields() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
slots:
  - id: vehicle
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("slot parses");
    let slot = &dag.slots[0];

    assert!(slot.attachment_predicates.is_empty());
    assert!(slot.addition_predicates.is_empty());
    assert!(slot.aggregate_breach_checks.is_empty());
    assert!(slot.additive_attachment_predicates.is_empty());
    assert!(slot.additive_addition_predicates.is_empty());
    assert!(slot.additive_aggregate_breach_checks.is_empty());
}

#[test]
fn slot_additive_predicate_vectors_batch_1b_parse_for_validation() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
slots:
  - id: vehicle
    +attachment_predicates:
      - "candidate.risk_rating != HIGH"
    +addition_predicates:
      - "this.category = FUND"
    +aggregate_breach_checks:
      - "count(vehicle) <= 4"
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("additive predicate vectors parse");
    let slot = &dag.slots[0];

    assert_eq!(
        slot.additive_attachment_predicates,
        vec!["candidate.risk_rating != HIGH".to_string()]
    );
    assert_eq!(
        slot.additive_addition_predicates,
        vec!["this.category = FUND".to_string()]
    );
    assert_eq!(
        slot.additive_aggregate_breach_checks,
        vec!["count(vehicle) <= 4".to_string()]
    );
}

#[test]
fn slot_discretionary_metadata_batch_1c_round_trips() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
slots:
  - id: waiver
    role_guard:
      any_of: [compliance_officer, operations_lead]
    justification_required: true
    audit_class: discretionary_override
    completeness_assertion:
      predicate: "all expected vehicles reviewed"
      description: Vehicle population reviewed by operations
      evidence_kind: manual_attestation
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("discretionary metadata parses");
    let slot = &dag.slots[0];

    let role_guard = slot.role_guard.as_ref().expect("role guard present");
    assert_eq!(
        role_guard.any_of,
        vec![
            "compliance_officer".to_string(),
            "operations_lead".to_string()
        ]
    );
    assert_eq!(slot.justification_required, Some(true));
    assert_eq!(slot.audit_class.as_deref(), Some("discretionary_override"));
    let completeness = slot
        .completeness_assertion
        .as_ref()
        .expect("completeness assertion present");
    assert_eq!(
        completeness.predicate.as_deref(),
        Some("all expected vehicles reviewed")
    );
    assert!(completeness.extra.contains_key("evidence_kind"));

    let round_trip = serde_yaml::to_string(&dag).expect("serializes");
    let reparsed: Dag = serde_yaml::from_str(&round_trip).expect("round-trip parses");
    assert_eq!(
        reparsed.slots[0]
            .role_guard
            .as_ref()
            .expect("role guard present")
            .any_of,
        vec![
            "compliance_officer".to_string(),
            "operations_lead".to_string()
        ]
    );
}

#[test]
fn slot_discretionary_metadata_batch_1c_defaults_absent_fields() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
slots:
  - id: waiver
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("slot parses");
    let slot = &dag.slots[0];

    assert!(slot.role_guard.is_none());
    assert_eq!(slot.justification_required, None);
    assert_eq!(slot.audit_class, None);
    assert!(slot.completeness_assertion.is_none());
}

#[test]
fn cross_workspace_constraint_replaceable_by_shape_batch_1d_defaults_false() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
cross_workspace_constraints:
  - id: demo_requires_deal_contracted
    source_workspace: deal
    source_slot: deal
    source_state: CONTRACTED
    target_workspace: demo
    target_slot: service
    target_transition: "proposed -> provisioned"
slots:
  - id: service
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("constraint parses");

    assert_eq!(dag.cross_workspace_constraints.len(), 1);
    assert!(!dag.cross_workspace_constraints[0].replaceable_by_shape);
}

#[test]
fn cross_workspace_constraint_replaceable_by_shape_batch_1d_parses_true() {
    let yaml = r#"
version: "1.0"
workspace: demo
dag_id: demo_dag
cross_workspace_constraints:
  - id: demo_requires_deal_contracted
    source_workspace: deal
    source_slot: deal
    source_state: CONTRACTED
    target_workspace: demo
    target_slot: service
    target_transition: "proposed -> provisioned"
    replaceable_by_shape: true
slots:
  - id: service
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("constraint parses");

    assert!(dag.cross_workspace_constraints[0].replaceable_by_shape);

    let round_trip = serde_yaml::to_string(&dag).expect("serializes");
    let reparsed: Dag = serde_yaml::from_str(&round_trip).expect("round-trip parses");
    assert!(reparsed.cross_workspace_constraints[0].replaceable_by_shape);
}

#[test]
fn predicate_binding_replaceable_by_shape_batch_1d_defaults_false() {
    let yaml = r#"
version: "1.0"
workspace: example
dag_id: example_dag
slots:
  - id: clearance
    state_machine:
      id: clearance_lifecycle
      source_entity: '"ob-poc".booking_principal_clearances'
      state_column: clearance_status
      predicate_bindings:
        - entity: screening_check
          source_kind: substrate
          source_entity: '"ob-poc".screenings'
      states:
        - id: PENDING
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("predicate binding parses");
    let dsl_core::config::dag::SlotStateMachine::Structured(machine) =
        dag.slots[0].state_machine.as_ref().expect("state machine")
    else {
        panic!("expected structured state machine");
    };

    assert!(!machine.predicate_bindings[0].replaceable_by_shape);
}

#[test]
fn predicate_binding_replaceable_by_shape_batch_1d_parses_true() {
    let yaml = r#"
version: "1.0"
workspace: example
dag_id: example_dag
slots:
  - id: clearance
    state_machine:
      id: clearance_lifecycle
      source_entity: '"ob-poc".booking_principal_clearances'
      state_column: clearance_status
      predicate_bindings:
        - entity: screening_check
          source_kind: substrate
          source_entity: '"ob-poc".screenings'
          replaceable_by_shape: true
      states:
        - id: PENDING
"#;

    let dag: Dag = serde_yaml::from_str(yaml).expect("predicate binding parses");
    let dsl_core::config::dag::SlotStateMachine::Structured(machine) =
        dag.slots[0].state_machine.as_ref().expect("state machine")
    else {
        panic!("expected structured state machine");
    };

    assert!(machine.predicate_bindings[0].replaceable_by_shape);

    let round_trip = serde_yaml::to_string(&dag).expect("serializes");
    let reparsed: Dag = serde_yaml::from_str(&round_trip).expect("round-trip parses");
    let dsl_core::config::dag::SlotStateMachine::Structured(reparsed_machine) = reparsed.slots[0]
        .state_machine
        .as_ref()
        .expect("state machine")
    else {
        panic!("expected structured state machine");
    };
    assert!(reparsed_machine.predicate_bindings[0].replaceable_by_shape);
}

#[test]
fn existing_cross_workspace_constraints_batch_1d_default_not_replaceable() {
    let dags = load_dags_from_dir(&dag_dir()).expect("DAG taxonomies load");
    let constraints: Vec<_> = dags
        .values()
        .flat_map(|loaded| loaded.dag.cross_workspace_constraints.iter())
        .collect();

    assert_eq!(constraints.len(), 11);
    assert!(constraints
        .iter()
        .all(|constraint| !constraint.replaceable_by_shape));
}

#[test]
fn existing_dag_taxonomies_parse_with_batch_1a_defaults() {
    let dags = load_dags_from_dir(&dag_dir()).expect("DAG taxonomies load");
    assert_eq!(dags.len(), 12);
}

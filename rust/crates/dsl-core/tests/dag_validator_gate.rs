use dsl_core::config::{
    entity_kinds_from_taxonomy_yaml, load_dags_from_dir,
    validate_constellation_map_dir_schema_coordination,
    validate_constellation_map_dir_schema_coordination_strict,
    validate_constellation_map_schema_coordination, validate_dags_with_context, Dag, DagError,
    DagValidationContext, DagWarning, LoadedDag,
};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

fn dag_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/sem_os_seeds/dag_taxonomies")
}

fn constellation_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/sem_os_seeds/constellation_maps")
}

fn ontology_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/ontology")
}

fn loaded(workspace: &str, yaml: &str) -> BTreeMap<String, LoadedDag> {
    let dag: Dag = serde_yaml::from_str(yaml).expect("DAG parses");
    BTreeMap::from([(
        workspace.to_string(),
        LoadedDag {
            source_path: PathBuf::new(),
            dag,
        },
    )])
}

fn validate_one(workspace: &str, yaml: &str, known_entity_kinds: &[&str]) -> Vec<DagError> {
    let context = DagValidationContext {
        known_entity_kinds: known_entity_kinds
            .iter()
            .map(|kind| kind.to_string())
            .collect::<HashSet<_>>(),
    };
    validate_dags_with_context(&loaded(workspace, yaml), &context).errors
}

#[test]
fn open_closure_without_completeness_assertion_errors() {
    let errors = validate_one(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: vehicle
    closure: open
"#,
        &[],
    );

    assert!(errors.iter().any(|error| matches!(
        error,
        DagError::OpenClosureMissingCompletenessAssertion { .. }
    )));
}

#[test]
fn eligibility_unknown_entity_kind_errors_when_context_supplied() {
    let errors = validate_one(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: vehicle
    eligibility:
      entity_kinds: [company, invented_kind]
"#,
        &["company"],
    );

    assert!(errors.iter().any(|error| matches!(
        error,
        DagError::EligibilityEntityKindUnknown { entity_kind, .. }
            if entity_kind == "invented_kind"
    )));
}

#[test]
fn entry_state_must_exist_in_inline_state_machine() {
    let errors = validate_one(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: evidence
    entry_state: PENDING
    state_machine:
      id: evidence_lifecycle
      states:
        - id: UPLOADED
          entry: true
"#,
        &[],
    );

    assert!(errors.iter().any(|error| matches!(
        error,
        DagError::EntryStateUnknown {
            slot_id,
            entry_state,
            ..
        } if slot_id == "evidence" && entry_state == "PENDING"
    )));
}

#[test]
fn entity_taxonomy_yaml_provides_known_entity_kinds() {
    let path = ontology_dir().join("entity_taxonomy.yaml");
    let yaml = std::fs::read_to_string(&path).expect("entity taxonomy readable");
    let kinds = entity_kinds_from_taxonomy_yaml(&yaml).expect("entity taxonomy parses");

    assert!(kinds.contains("cbu"));
    assert!(kinds.contains("proper_person"));
}

#[test]
fn gate_predicate_parse_errors_are_reported() {
    let errors = validate_one(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: vehicle
    attachment_predicates:
      - "every required"
"#,
        &[],
    );

    assert!(errors
        .iter()
        .any(|error| matches!(error, DagError::GatePredicateParseError { field, .. } if field == "attachment_predicates")));
}

#[test]
fn predicate_binding_without_declared_carrier_is_reported() {
    let errors = validate_one(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: vehicle
    state_machine:
      id: vehicle_lifecycle
      predicate_bindings:
        - entity: review
          source_kind: dag_entity
      states:
        - id: PENDING
          entry: true
        - id: APPROVED
          green_when: "review.state = APPROVED"
"#,
        &[],
    );

    assert!(errors.iter().any(|error| matches!(
        error,
        DagError::PredicateBindingCarrierMissing {
            slot_id,
            state_id,
            entity_kind,
            ..
        } if slot_id == "vehicle" && state_id == "APPROVED" && entity_kind == "review"
    )));
}

#[test]
fn additive_predicate_sigil_is_rejected_in_dag_taxonomy() {
    let errors = validate_one(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: vehicle
    +attachment_predicates:
      - "review exists"
"#,
        &[],
    );

    assert!(errors
        .iter()
        .any(|error| matches!(error, DagError::AdditivePredicateSigilForbidden { field, .. } if field == "+attachment_predicates")));
}

#[test]
fn additive_predicate_sigil_is_rejected_in_constellation_map() {
    let report = validate_constellation_map_schema_coordination(
        &BTreeMap::new(),
        "demo_constellation.yaml",
        r#"
constellation: demo.map
jurisdiction: ALL
slots:
  vehicle:
    type: entity
    cardinality: optional
    +attachment_predicates:
      - "review exists"
"#,
    );

    assert!(report
        .errors
        .iter()
        .any(|error| matches!(error, DagError::AdditivePredicateSigilForbidden { field, .. } if field == "+attachment_predicates")));
}

#[test]
fn schema_coordination_warns_on_gate_field_drift() {
    let dags = loaded(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: vehicle
    closure: closed_bounded
"#,
    );
    let report = validate_constellation_map_schema_coordination(
        &dags,
        "demo_constellation.yaml",
        r#"
constellation: demo.map
jurisdiction: ALL
slots:
  vehicle:
    type: entity
    cardinality: optional
    closure: open
"#,
    );

    assert!(report.warnings.iter().any(|warning| matches!(
        warning,
        DagWarning::SchemaCoordinationSlotFieldDrift { field, .. }
            if field == "closure"
    )));
}

#[test]
fn schema_coordination_warns_on_state_machine_mismatch() {
    let dags = loaded(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: vehicle
    state_machine:
      id: vehicle_lifecycle
      states:
        - id: DRAFT
"#,
    );
    let report = validate_constellation_map_schema_coordination(
        &dags,
        "demo_constellation.yaml",
        r#"
constellation: demo.map
jurisdiction: ALL
slots:
  vehicle:
    type: entity
    cardinality: optional
    state_machine: other_lifecycle
"#,
    );

    assert!(report.warnings.iter().any(|warning| matches!(
        warning,
        DagWarning::SchemaCoordinationStateMachineMismatch { .. }
    )));
}

#[test]
fn strict_schema_coordination_promotes_undocumented_warning_to_error() {
    let dags = loaded(
        "demo",
        r#"
workspace: demo
dag_id: demo
slots:
  - id: vehicle
    closure: closed_bounded
"#,
    );
    let mut report = validate_constellation_map_schema_coordination(
        &dags,
        "demo_constellation.yaml",
        r#"
constellation: demo.map
jurisdiction: ALL
slots:
  vehicle:
    type: entity
    cardinality: optional
    closure: open
"#,
    );

    dsl_core::config::dag_validator::harden_schema_coordination_warnings(&mut report, &[]);

    assert!(report.warnings.is_empty(), "{:#?}", report.warnings);
    assert!(report.errors.iter().any(|error| matches!(
        error,
        DagError::SchemaCoordinationSlotFieldDrift { field, .. }
            if field == "closure"
    )));
}

#[test]
fn authored_seed_constellation_maps_match_documented_schema_coordination_warnings() {
    let dags = load_dags_from_dir(&dag_dir()).expect("DAG taxonomies load");
    let report = validate_constellation_map_dir_schema_coordination(&dags, &constellation_dir())
        .expect("constellation map directory validates");

    assert!(
        report.errors.is_empty(),
        "schema-coordination errors: {:#?}",
        report.errors
    );
    assert!(report.warnings.is_empty(), "{:#?}", report.warnings);
}

#[test]
fn strict_authored_seed_schema_coordination_preserves_known_deferred_only() {
    let dags = load_dags_from_dir(&dag_dir()).expect("DAG taxonomies load");
    let report =
        validate_constellation_map_dir_schema_coordination_strict(&dags, &constellation_dir(), &[])
            .expect("constellation map directory validates");

    assert!(
        report.errors.is_empty(),
        "strict schema-coordination errors: {:#?}",
        report.errors
    );
    assert!(report.warnings.is_empty(), "{:#?}", report.warnings);
}

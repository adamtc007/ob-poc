use dsl_core::config::{
    green_when_coverage_for_dag, green_when_coverage_for_dags, green_when_coverage_summary,
    load_dags_from_dir, ConfigLoader, Dag, GreenWhenExclusionReason, VerbFlavour, VerbsConfig,
};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

fn seed_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/sem_os_seeds")
        .join(relative)
}

fn discretionary_verbs(config: &VerbsConfig) -> HashSet<String> {
    config
        .domains
        .iter()
        .flat_map(|(domain_name, domain)| {
            domain
                .verbs
                .iter()
                .filter(|(_, verb)| verb.flavour == Some(VerbFlavour::Discretionary))
                .map(move |(verb_name, _)| format!("{domain_name}.{verb_name}"))
        })
        .collect()
}

#[test]
fn synthetic_coverage_excludes_entry_source_and_discretionary_destinations() {
    let yaml = r#"
version: 1.4
workspace: demo
dag_id: demo_dag
slots:
  - id: item
    state_machine:
      id: item_lifecycle
      states:
        - id: DRAFT
          entry: true
        - id: READY
          green_when: "review exists"
        - id: REJECTED
        - id: ARCHIVED
      transitions:
        - from: DRAFT
          to: READY
          via: item.ready
        - from: READY
          to: REJECTED
          via: item.reject
        - from: READY
          to: ARCHIVED
          via: item.archive
"#;
    let dag: Dag = serde_yaml::from_str(yaml).expect("synthetic DAG parses");
    let discretionary = HashSet::from(["item.reject".to_string()]);

    let rows = green_when_coverage_for_dag("demo", &dag, &discretionary);
    let draft = rows.iter().find(|row| row.state_id == "DRAFT").unwrap();
    let ready = rows.iter().find(|row| row.state_id == "READY").unwrap();
    let rejected = rows.iter().find(|row| row.state_id == "REJECTED").unwrap();
    let archived = rows.iter().find(|row| row.state_id == "ARCHIVED").unwrap();

    assert_eq!(
        draft.exclusion_reason,
        Some(GreenWhenExclusionReason::EntryState)
    );
    assert!(ready.candidate);
    assert!(ready.has_green_when);
    assert_eq!(
        rejected.exclusion_reason,
        Some(GreenWhenExclusionReason::DiscretionaryDestination)
    );
    assert!(archived.candidate);
    assert!(!archived.has_green_when);

    let summary = green_when_coverage_summary(&rows);
    assert_eq!(summary.candidate_states, 2);
    assert_eq!(summary.covered_candidate_states, 1);
    assert_eq!(summary.missing_candidate_states, 1);
}

#[test]
fn real_dag_green_when_coverage_baseline_is_explicit() {
    let (dags, discretionary) = real_dags_and_discretionary_verbs();
    let rows = green_when_coverage_for_dags(&dags, &discretionary);
    let summary = green_when_coverage_summary(&rows);

    assert!(
        summary.total_states >= 332,
        "state count regressed below baseline"
    );
    assert!(
        summary.candidate_states >= 196,
        "candidate count regressed below baseline"
    );
    assert!(
        summary.covered_candidate_states >= 12,
        "green_when coverage regressed below baseline"
    );
    assert_eq!(
        summary.candidate_states - summary.covered_candidate_states,
        summary.missing_candidate_states
    );
}

#[test]
fn real_dag_green_when_coverage_is_tracked_per_workspace() {
    let (dags, discretionary) = real_dags_and_discretionary_verbs();
    let expected = BTreeMap::from([
        ("book_setup".to_string(), (8, 6, 0, 6)),
        ("booking_principal".to_string(), (7, 2, 0, 2)),
        ("catalogue".to_string(), (5, 3, 0, 3)),
        ("cbu".to_string(), (69, 37, 4, 33)),
        ("deal".to_string(), (54, 34, 1, 33)),
        ("instrument_matrix".to_string(), (55, 27, 2, 25)),
        ("kyc".to_string(), (87, 56, 4, 52)),
        ("lifecycle_resources".to_string(), (11, 7, 0, 7)),
        ("onboarding_request".to_string(), (0, 0, 0, 0)),
        ("product_maintenance".to_string(), (10, 3, 0, 3)),
        ("semos_maintenance".to_string(), (26, 21, 1, 20)),
        ("session_bootstrap".to_string(), (0, 0, 0, 0)),
    ]);

    let rows = green_when_coverage_for_dags(&dags, &discretionary);
    let mut actual = BTreeMap::new();
    for (workspace, _) in dags {
        let workspace_rows: Vec<_> = rows
            .iter()
            .filter(|row| row.workspace == workspace)
            .cloned()
            .collect();
        let summary = green_when_coverage_summary(&workspace_rows);
        actual.insert(
            workspace,
            (
                summary.total_states,
                summary.candidate_states,
                summary.covered_candidate_states,
                summary.missing_candidate_states,
            ),
        );
    }

    for (workspace, (total, candidate, covered, missing)) in expected {
        let Some(actual) = actual.get(&workspace) else {
            panic!("{workspace} missing from green_when coverage");
        };
        assert!(
            actual.0 >= total,
            "{workspace} total state count regressed below baseline"
        );
        assert!(
            actual.1 >= candidate,
            "{workspace} candidate count regressed below baseline"
        );
        assert!(
            actual.2 >= covered,
            "{workspace} green_when coverage regressed below baseline"
        );
        assert_eq!(
            actual.1 - actual.2,
            actual.3,
            "{workspace} missing coverage total is inconsistent"
        );
        assert_eq!(
            candidate - covered,
            missing,
            "{workspace} baseline fixture is internally inconsistent"
        );
    }
}

fn real_dags_and_discretionary_verbs() -> (BTreeMap<String, Dag>, HashSet<String>) {
    let loaded = load_dags_from_dir(&seed_path("dag_taxonomies")).expect("DAG taxonomies load");
    let dags: BTreeMap<_, _> = loaded
        .into_iter()
        .map(|(workspace, loaded)| (workspace, loaded.dag))
        .collect();
    let verbs = ConfigLoader::from_env()
        .load_verbs()
        .expect("real verb catalogue loads");
    (dags, discretionary_verbs(&verbs))
}

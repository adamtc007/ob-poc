use std::{collections::HashSet, fs, path::Path};

use chrono::Utc;
use dsl_core::{
    green_when_coverage_for_dags, green_when_coverage_summary, load_dags_from_dir,
    parse_green_when, validate_verbs_config, ConfigLoader, SlotStateMachine, ValidationContext,
    VerbFlavour,
};
use sem_os_policy::domain_pack::{
    refresh_domain_pack_taxonomy_with_index, reload_domain_pack_taxonomy_from_yaml,
    reload_index_entry_from_reload, DomainPackManifest, DomainPackRefreshAction,
    DomainPackReloadStatus,
};

fn config_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config")
}

fn yaml_values(dir: &Path) -> Vec<serde_yaml::Value> {
    let mut paths = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("yaml" | "yml")
            )
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            serde_yaml::from_str(
                &fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
            )
            .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
        })
        .collect()
}

#[test]
fn application_domain_pack_manifests_parse_and_validate() {
    for (file, expected_id) in [
        ("ob_poc_kyc.yaml", "ob-poc.kyc"),
        ("ob_poc_cbu.yaml", "ob-poc.cbu"),
    ] {
        let path = config_root().join("sem_os_seeds/domain_packs").join(file);
        let manifest: DomainPackManifest =
            serde_yaml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let report = manifest.validate();
        assert!(report.valid, "{}: {:?}", path.display(), report.diagnostics);
        assert_eq!(manifest.pack_id, expected_id);
    }
}

#[test]
fn cbu_taxonomy_reload_is_idempotent() {
    let root = config_root();
    let first = reload_domain_pack_taxonomy_from_yaml(&root, "ob-poc.cbu").unwrap();
    let second = reload_domain_pack_taxonomy_from_yaml(&root, "ob-poc.cbu").unwrap();
    assert_eq!(first, second);
    assert!(first.surfaces.contains_key("dag:cbu_dag"));
    assert!(first.surfaces.contains_key("pack:cbu-maintenance"));
}

#[test]
fn unchanged_source_skips_refresh() {
    let root = config_root();
    let now = Utc::now();
    let reload = reload_domain_pack_taxonomy_from_yaml(&root, "ob-poc.cbu").unwrap();
    let index = reload_index_entry_from_reload(&root, &reload, now, DomainPackReloadStatus::Loaded)
        .unwrap();
    let plan =
        refresh_domain_pack_taxonomy_with_index(&root, "ob-poc.cbu", Some(&index), false, now)
            .unwrap();
    assert_eq!(plan.action, DomainPackRefreshAction::Skip);
    assert!(plan.reload.is_none());
}

#[test]
fn changed_fingerprint_with_same_surface_updates_only_index() {
    let root = config_root();
    let now = Utc::now();
    let reload = reload_domain_pack_taxonomy_from_yaml(&root, "ob-poc.cbu").unwrap();
    let mut index =
        reload_index_entry_from_reload(&root, &reload, now, DomainPackReloadStatus::Loaded)
            .unwrap();
    index.source_fingerprints[0].size_bytes += 1;
    let plan =
        refresh_domain_pack_taxonomy_with_index(&root, "ob-poc.cbu", Some(&index), false, now)
            .unwrap();
    assert_eq!(plan.action, DomainPackRefreshAction::IndexOnly);
    assert!(plan.reload.is_some());
}

#[test]
fn missing_index_requires_publish() {
    let plan = refresh_domain_pack_taxonomy_with_index(
        config_root(),
        "ob-poc.cbu",
        None,
        false,
        Utc::now(),
    )
    .unwrap();
    assert_eq!(plan.action, DomainPackRefreshAction::PublishRequired);
    assert_eq!(
        plan.index_entry.status,
        DomainPackReloadStatus::PublishRequired
    );
}

#[test]
fn every_dsl_pack_and_dag_has_domain_pack_ownership() {
    let root = config_root();
    let reloads = yaml_values(&root.join("sem_os_seeds/domain_packs"))
        .into_iter()
        .map(|yaml| {
            let pack_id = yaml
                .get("pack_id")
                .and_then(serde_yaml::Value::as_str)
                .expect("domain pack declares pack_id")
                .to_owned();
            let reload = reload_domain_pack_taxonomy_from_yaml(&root, &pack_id)
                .unwrap_or_else(|error| panic!("reload {pack_id}: {error:#}"));
            (pack_id, reload)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut owned_packs = HashSet::new();
    let mut owned_dags = HashSet::new();
    for reload in reloads.values() {
        owned_packs.extend(reload.manifest.owned_packs.iter().cloned());
        owned_dags.extend(reload.manifest.owned_dags.iter().cloned());
    }

    let actual_packs = yaml_values(&root.join("packs"))
        .into_iter()
        .filter_map(|yaml| {
            yaml.get("id")
                .and_then(serde_yaml::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<HashSet<_>>();
    let actual_dags = yaml_values(&root.join("sem_os_seeds/dag_taxonomies"))
        .into_iter()
        .filter_map(|yaml| {
            yaml.get("dag_id")
                .and_then(serde_yaml::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<HashSet<_>>();

    assert_eq!(
        actual_packs.difference(&owned_packs).collect::<Vec<_>>(),
        Vec::<&String>::new()
    );
    assert_eq!(
        actual_dags.difference(&owned_dags).collect::<Vec<_>>(),
        Vec::<&String>::new()
    );
}

#[test]
fn application_verb_catalogue_is_complete_and_well_formed() {
    let config = ConfigLoader::new(config_root().to_string_lossy())
        .load_verbs()
        .expect("application verb catalogue loads through public DSL API");
    let total = config
        .domains
        .values()
        .map(|domain| domain.verbs.len())
        .sum::<usize>();
    assert!(
        total >= 1_250,
        "verb count regressed below baseline: {total}"
    );

    let report = validate_verbs_config(
        &config,
        &ValidationContext {
            require_flavour: true,
            ..ValidationContext::default()
        },
    );
    assert!(
        report.well_formedness.is_empty(),
        "application catalogue validation errors: {:#?}",
        report.well_formedness
    );

    let mut discretionary = 0;
    for (domain_name, domain) in &config.domains {
        for (verb_name, verb) in &domain.verbs {
            let fqn = format!("{domain_name}.{verb_name}");
            assert!(verb.flavour.is_some(), "{fqn} is missing flavour");
            if verb.flavour == Some(VerbFlavour::Discretionary) {
                discretionary += 1;
                let role_guard = verb
                    .role_guard
                    .as_ref()
                    .unwrap_or_else(|| panic!("{fqn} is missing role_guard"));
                assert!(
                    !role_guard.any_of.is_empty() || !role_guard.all_of.is_empty(),
                    "{fqn} has an empty role_guard"
                );
                assert!(
                    verb.audit_class
                        .as_ref()
                        .is_some_and(|value| !value.is_empty()),
                    "{fqn} is missing audit_class"
                );
            }
            if verb.flavour == Some(VerbFlavour::Tollgate) {
                assert!(
                    verb.crud.is_none()
                        && verb.handler.is_none()
                        && verb.graph_query.is_none()
                        && verb.durable.is_none(),
                    "{fqn} has tollgate flavour with a non-empty executable body"
                );
            }
        }
    }
    assert!(
        discretionary >= 150,
        "discretionary verb count regressed below baseline: {discretionary}"
    );
}

#[test]
fn application_dag_predicates_and_green_when_coverage_are_qualified() {
    let loaded = load_dags_from_dir(&config_root().join("sem_os_seeds/dag_taxonomies"))
        .expect("application DAG taxonomies load through public DSL API");
    let dags = loaded
        .iter()
        .map(|(workspace, loaded)| (workspace.clone(), loaded.dag.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let verbs = ConfigLoader::new(config_root().to_string_lossy())
        .load_verbs()
        .expect("application verb catalogue loads");
    let discretionary = verbs
        .domains
        .iter()
        .flat_map(|(domain_name, domain)| {
            domain
                .verbs
                .iter()
                .filter(|(_, verb)| verb.flavour == Some(VerbFlavour::Discretionary))
                .map(move |(verb_name, _)| format!("{domain_name}.{verb_name}"))
        })
        .collect::<HashSet<_>>();

    let rows = green_when_coverage_for_dags(&dags, &discretionary);
    let summary = green_when_coverage_summary(&rows);
    assert!(
        summary.total_states >= 332,
        "state count regressed: {summary:?}"
    );
    assert!(
        summary.candidate_states >= 196,
        "candidate count regressed: {summary:?}"
    );
    assert!(
        summary.covered_candidate_states >= 6,
        "green_when coverage regressed: {summary:?}"
    );

    let mut predicate_count = 0;
    for loaded in loaded.values() {
        for slot in &loaded.dag.slots {
            let Some(SlotStateMachine::Structured(machine)) = &slot.state_machine else {
                continue;
            };
            for state in &machine.states {
                let Some(predicate) = state.green_when.as_deref() else {
                    continue;
                };
                if predicate.trim().is_empty() {
                    continue;
                }
                parse_green_when(predicate).unwrap_or_else(|error| {
                    panic!(
                        "{} / {} / {} has invalid green_when `{predicate}`: {error}",
                        loaded.dag.dag_id, slot.id, state.id
                    )
                });
                predicate_count += 1;
            }
        }
    }
    assert!(
        predicate_count >= 12,
        "green_when predicate count regressed: {predicate_count}"
    );
}

/// WS-2.A (EOP-PLAN-SEM-RESOLVER-001, ruled 2026-08-06): the Designer
/// authoring plane is referenced by exact content-hash pin in TWO seed
/// declarations — the bpmn_dag workspace_root slot annotation and the
/// ob-poc.bpmn-ops domain pack's typed extension point. This gate
/// refuses silent drift between them and refuses a malformed pin
/// (empty / non-64-hex — a pin that names nothing is a name-check,
/// not a pin). The artifact-hash truth link is compiler-verified on
/// the bpmn-lite side (bpmn-semantic-pack.lock); this side proves the
/// declared copies agree and are well-formed.
#[test]
fn bpmn_authoring_plane_pin_is_consistent_and_well_formed() {
    let root = config_root().join("sem_os_seeds");

    let dag: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(root.join("dag_taxonomies/bpmn_dag.yaml")).expect("read bpmn_dag"),
    )
    .expect("parse bpmn_dag");
    let slot = &dag["slots"][0];
    assert_eq!(slot["id"].as_str(), Some("workspace_root"));
    let plane = &slot["authoring_plane"];
    let pack_id = plane["pack_id"].as_str().expect("authoring_plane.pack_id");
    let version = plane["pack_version"]
        .as_str()
        .expect("authoring_plane.pack_version");
    let artifact = plane["artifact_sha256"]
        .as_str()
        .expect("authoring_plane.artifact_sha256");
    for (label, hash) in [
        ("artifact_sha256", artifact),
        (
            "source_sha256",
            plane["source_sha256"].as_str().expect("source_sha256"),
        ),
    ] {
        assert_eq!(hash.len(), 64, "{label} must be 64 hex chars");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{label} must be lowercase hex"
        );
    }
    assert!(!pack_id.trim().is_empty() && !version.trim().is_empty());

    let pack: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(root.join("domain_packs/ob_poc_bpmn.yaml")).expect("read ob_poc_bpmn"),
    )
    .expect("parse ob_poc_bpmn");
    let points = pack["typed_extension_points"]
        .as_sequence()
        .expect("typed_extension_points sequence");
    let plane_ref = points
        .iter()
        .find(|p| p["extension_kind"].as_str() == Some("authoring_plane_ref"))
        .expect("ob_poc_bpmn.yaml must declare the authoring_plane_ref extension point");
    assert_eq!(
        plane_ref["implementation_ref"].as_str(),
        Some(format!("semantic-pack://{pack_id}@{version}#sha256:{artifact}").as_str()),
        "domain-pack extension ref must agree exactly with the DAG slot pin"
    );
}

/// WS-2.D (EOP-PLAN-SEM-RESOLVER-001, review P1): verb state-precondition
/// gate over the DAG-derived universe. The DAG is normative — for every
/// verb reached `via:` a structured transition, its from-states ARE its
/// legal `requires_states`. Two checks, both fail-closed:
///   1. DRIFT: a verb that declares `lifecycle.requires_states` must
///      declare exactly the DAG-derived from-state union — a declared
///      set the DAG contradicts is a live inconsistency, not a style
///      issue (the runtime enforces the YAML side at execute_verb_in_scope).
///   2. RATCHET: the count of DAG-transition verbs declaring
///      requires_states never drops below the recorded floor. The floor
///      only rises as the derivable backlog (~184 verbs) is backfilled —
///      scoped to slot_state_table-mapped slots to avoid the
///      NoSlotMapping runtime trap.
#[test]
fn dag_transition_verbs_requires_states_drift_and_ratchet() {
    use std::collections::{BTreeMap, BTreeSet};

    // Declared side: verb FQN -> requires_states from config/verbs/**.
    let mut declared: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut all_verbs: BTreeSet<String> = BTreeSet::new();
    let mut stack = vec![config_root().join("verbs")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read verbs dir").filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !matches!(path.extension().and_then(|e| e.to_str()), Some("yaml" | "yml")) {
                continue;
            }
            let value: serde_yaml::Value =
                serde_yaml::from_str(&fs::read_to_string(&path).expect("read verb file"))
                    .expect("parse verb file");
            let Some(domains) = value.get("domains").and_then(|d| d.as_mapping()) else {
                continue;
            };
            for (domain, body) in domains {
                let Some(verbs) = body.get("verbs").and_then(|v| v.as_mapping()) else {
                    continue;
                };
                for (verb, spec) in verbs {
                    let fqn = format!(
                        "{}.{}",
                        domain.as_str().unwrap_or_default(),
                        verb.as_str().unwrap_or_default()
                    );
                    all_verbs.insert(fqn.clone());
                    let states = spec
                        .get("lifecycle")
                        .and_then(|l| l.get("requires_states"))
                        .and_then(|r| r.as_sequence())
                        .map(|seq| {
                            seq.iter()
                                .filter_map(|s| s.as_str().map(str::to_owned))
                                .collect::<BTreeSet<_>>()
                        })
                        .unwrap_or_default();
                    if !states.is_empty() {
                        declared.insert(fqn, states);
                    }
                }
            }
        }
    }

    // Derived side: verb FQN -> union of DAG transition from-states.
    let mut derived: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let dag_dir = config_root().join("sem_os_seeds/dag_taxonomies");
    for entry in fs::read_dir(&dag_dir).expect("read dag dir").filter_map(Result::ok) {
        let path = entry.path();
        if !matches!(path.extension().and_then(|e| e.to_str()), Some("yaml" | "yml")) {
            continue;
        }
        let dag: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(&path).expect("read dag")).expect("parse dag");
        let Some(slots) = dag.get("slots").and_then(|s| s.as_sequence()) else {
            continue;
        };
        for slot in slots {
            let Some(transitions) = slot
                .get("state_machine")
                .and_then(|m| m.get("transitions"))
                .and_then(|t| t.as_sequence())
            else {
                continue;
            };
            for transition in transitions {
                let Some(via) = transition.get("via").and_then(|v| v.as_str()) else {
                    continue;
                };
                let via = via.trim();
                if !all_verbs.contains(via) {
                    continue; // macro prose / table names — not verbs
                }
                // Some DAGs author multi-state froms as one
                // parenthesized string: "(DRAFT, ACTIVE)". Expand.
                let expand = |raw: &str, set: &mut BTreeSet<String>| {
                    let raw = raw.trim();
                    if let Some(inner) =
                        raw.strip_prefix('(').and_then(|r| r.strip_suffix(')'))
                    {
                        set.extend(
                            inner
                                .split(',')
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned),
                        );
                    } else if !raw.is_empty() {
                        set.insert(raw.to_owned());
                    }
                };
                let mut froms = BTreeSet::new();
                match transition.get("from") {
                    Some(serde_yaml::Value::String(s)) => expand(s, &mut froms),
                    Some(serde_yaml::Value::Sequence(seq)) => {
                        for s in seq.iter().filter_map(|s| s.as_str()) {
                            expand(s, &mut froms);
                        }
                    }
                    _ => {}
                }
                if !froms.is_empty() {
                    derived.entry(via.to_owned()).or_default().extend(froms);
                }
            }
        }
    }

    // 1. DRIFT — declared must equal derived wherever both exist.
    let mut drift = Vec::new();
    for (verb, declared_states) in &declared {
        if let Some(derived_states) = derived.get(verb) {
            if declared_states != derived_states {
                drift.push(format!(
                    "{verb}: declares {declared_states:?} but the normative DAG derives {derived_states:?}"
                ));
            }
        }
    }
    assert!(
        drift.is_empty(),
        "requires_states drift against the normative DAG:\n{}",
        drift.join("\n")
    );

    // 2. RATCHET — floors only rise. Recorded 2026-08-06.
    let universe = derived.len();
    let covered = derived.keys().filter(|v| declared.contains_key(*v)).count();
    assert!(
        universe >= 200,
        "DAG-derivable verb universe shrank unexpectedly: {universe}"
    );
    assert!(
        covered >= 80,
        "requires_states coverage regressed below the recorded floor \
         (batch 1 applied 2026-08-06): {covered} of {universe}"
    );
}

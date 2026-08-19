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
    // Verb FQN -> (target_workspace, target_slot), from the verb's own
    // `transition_args` when declared. R3 (EOP-PLAN-GAMEBOARD-001,
    // 2026-08-18) found this test's original "union every from-state across
    // every DAG slot that happens to name this verb as `via:`" derivation
    // silently mixes in incidental transitions from OTHER slots — e.g.
    // `entity-workstream.complete` is also `via:` on a transition inside
    // `entity_kyc` (a *derived* rollup slot, `state_column: "(derived from
    // entity_workstreams.status + stream projections)"`, no real backing
    // column), whose `verified`/`approved` states have nothing to do with
    // this verb's REAL target slot (`kyc.entity_workstream`, confirmed via
    // its own `transition_args`). Scoping the derivation to the verb's own
    // declared target slot (falling back to the old any-slot union only for
    // the minority of verbs with no `transition_args` at all) is what R3's
    // register-verified corrections are consistent with; the un-scoped
    // union is what let the original WS-2.D batch 1 pass silently encode
    // states from the wrong slot in the first place.
    let mut verb_target: BTreeMap<String, (String, String)> = BTreeMap::new();
    // Manual overrides for verbs with no `transition_args` block at all —
    // same targets `gameboard_r3_declaration_register.rs`'s
    // `manual_target_overrides()` derived (domain-name/sibling-verb
    // context; no mechanical derivation exists for these). Without this,
    // `entity-workstream.complete`, `governance.record-review`, and
    // `service-resource.decommission` fall through the scoping guard below
    // unfiltered, re-polluting their derived sets with incidental
    // transitions from unrelated slots (e.g. `entity_kyc`'s derived-rollup
    // `verified -> approved via: entity-workstream.complete`).
    for (fqn, ws, slot) in [
        ("entity-workstream.complete", "kyc", "entity_workstream"),
        ("entity-workstream.update-status", "kyc", "entity_workstream"),
        ("governance.record-review", "semos_maintenance", "changeset"),
        ("service-resource.decommission", "instrument_matrix", "service_resource"),
    ] {
        verb_target.insert(fqn.to_string(), (ws.to_string(), slot.to_string()));
    }
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
                        declared.insert(fqn.clone(), states);
                    }
                    if let Some((ws, slot)) = spec.get("transition_args").and_then(|ta| {
                        let ws = ta.get("target_workspace")?.as_str()?.to_owned();
                        let slot = ta.get("target_slot")?.as_str()?.to_owned();
                        Some((ws, slot))
                    }) {
                        verb_target.insert(fqn, (ws, slot));
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
        let workspace = dag.get("workspace").and_then(|w| w.as_str()).unwrap_or_default();
        let Some(slots) = dag.get("slots").and_then(|s| s.as_sequence()) else {
            continue;
        };
        for slot in slots {
            let slot_id = slot.get("id").and_then(|s| s.as_str()).unwrap_or_default();
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
                // Scope to the verb's own declared target slot when known
                // (see verb_target's doc above) — a verb incidentally named
                // as `via:` on some OTHER slot's transition (e.g. a derived
                // rollup slot sharing verb names with its source slot) must
                // not pollute this verb's real from-state set.
                if let Some((target_ws, target_slot)) = verb_target.get(via) {
                    if target_ws != workspace || target_slot != slot_id {
                        continue;
                    }
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

    // 2. RATCHET — floors only rise. Recorded 2026-08-06, universe floor
    // consciously lowered 2026-08-18 (EOP-PLAN-GAMEBOARD-001 R3 scoping
    // fix — see verb_target's doc comment above): the un-scoped "union
    // every from-state across every slot naming this verb as `via:`"
    // derivation didn't just mix in the entity_kyc rollup-slot pollution
    // R3 was built to fix — it also silently counted 27 verbs as
    // "DAG-derivable" whose OWN declared `transition_args` target doesn't
    // match where their transition actually lives in the DAG:
    //   - 26 (investor.*, holding.*, manco.*, entity.{identify,verify},
    //     kyc-case.approve-with-conditions) still declare
    //     `target_workspace: cbu` (or a stale `target_slot`) even though
    //     their slots (investor, investor_kyc, holding, manco,
    //     entity_proper_person) were relocated from cbu_dag.yaml into
    //     kyc_dag.yaml by the CBU⊥KYC decoupling "Cluster 4" move (see
    //     CLAUDE.md's CBU⊥KYC Domain Decoupling section) — the verb YAML's
    //     own `transition_args` was never updated to follow that move.
    //   - 1 (trading-profile.retire-template) has a target_slot mismatch
    //     unrelated to the Cluster 4 move.
    // These are real, live findings (transition_args feeds the v1.3
    // GateChecker enforcement path, not just this test) — NOT encoded here
    // because fixing them changes live gate-check behavior and needs its
    // own ruling, same discipline as R2 Stage 2. Scoping correctly drops
    // them from `derived` rather than crediting them on the strength of an
    // incidental, wrong-slot transition; the floor moves with the more
    // correct measurement instead of hiding it behind stale unscoped counts.
    let universe = derived.len();
    let covered = derived.keys().filter(|v| declared.contains_key(*v)).count();
    assert!(
        universe >= 191,
        "DAG-derivable verb universe shrank unexpectedly: {universe}"
    );
    assert!(
        // Floor consciously lowered 79 (2026-08-19, EOP-PLAN-GAMEBOARD-001):
        // trading-profile.create-draft's lifecycle block was removed
        // entirely (it was a broken declaration that could never have
        // worked — see this file's own dedicated pin comment above and
        // the verb's own YAML comment). One fewer "covered" verb is
        // correct, not a regression.
        covered >= 79,
        "requires_states coverage regressed below the recorded floor \
         (batch 1 applied 2026-08-06): {covered} of {universe}"
    );
}

// ── Verification-pass closure teeth (execution-confirmed, 2026-08-18) ──────
//
// Both defects below were CONFIRMED by live execution against the real
// production `runtime_registry()` and the real `data_designer` dev DB
// (`enforce_requires_states_precondition` called through a rolled-back
// `PgTransactionScope`, with `config/slot_state_table.yaml` seeded the SAME
// way `ob-poc-web::main` seeds it at startup — not inferred from source
// alone). Neither tooth below CLOSES its defect — both pin the EXACT
// currently-broken/dead set so the drift class this repo already tracks
// (K-G7-style "declared but not enforced") gets a fifth and sixth instance
// with real regression coverage instead of zero. Wiring either gap up for
// real is a separate, ratified change; when that happens, these assertions
// must shrink deliberately, not silently pass.

/// KNOWN GAP #5, R2 STAGE 1 CONSCIOUS EDIT (EOP-PLAN-GAMEBOARD-001,
/// 2026-08-18) — was `cross_slot_constraints_declared_but_zero_consumers_
/// is_the_known_gap`, RED (asserted zero consumers existed anywhere).
/// R2 Stage 1 landed a real, report-only evaluator
/// (`src/cross_slot_census.rs`) — this flips the test to pin the NEW
/// state: a consumer now exists and is evaluated, but two things are
/// still deliberately true and must stay true until their own separate,
/// ratified changes land:
///   1. `DagRegistry` (`crates/dsl-runtime/src/cross_workspace/dag_registry.rs`)
///      still does NOT index `cross_slot_constraints` — Stage 1 is
///      report-only and standalone by design (see that module's own doc:
///      wiring it into live per-transition dispatch shares a call site
///      with the in-flight Control-Plane Graduation program, paused
///      pending reconciliation, not bundled in here).
///   2. Only 3 of the 49 declared rules have a real hand-verified
///      Clean/Violated check (`cbu_validated_requires_evidence_set_verified`,
///      `cbu_validated_requires_commercial_client_entity`,
///      `deal_contracted_requires_bac_approved`); the rest are honestly
///      `NotYetImplemented`/`SchemaMismatch`, never silently dropped.
///
/// Live consequence, confirmed by the census this session (raw output
/// pasted in the R2 Stage 1 report, not reproduced here): the exact
/// `cbu_validated_requires_evidence_set_verified` /
/// `cbu_validated_requires_commercial_client_entity` blockers this
/// comment used to cite as unchecked are now REAL, LOGGED violations —
/// 93 and 78 VALIDATED CBUs respectively, live in the dev DB today.
/// Nothing refuses them (Stage 1 is report-only); Stage 2 (ratified
/// separately, after Adam reads the census) is what would start refusing.
///
/// RED→GREEN evidence for this conscious edit: before this change, `cargo
/// test --lib cross_slot_census` did not exist (no such module); after,
/// `cross_slot_census::tests::every_declared_cross_slot_constraint_is_evaluated`
/// passes and prints a 49-row census with 3 real verdicts, 2 schema
/// mismatches, 44 not-yet-implemented — pasted in this session's R2 Stage
/// 1 report.
#[test]
fn cross_slot_constraints_have_a_report_only_evaluator_not_yet_wired_to_dispatch() {
    let dag_dir = config_root().join("sem_os_seeds/dag_taxonomies");
    let mut declared_ids: Vec<String> = Vec::new();
    let mut paths: Vec<_> = fs::read_dir(&dag_dir)
        .expect("read dag_taxonomies dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("yaml" | "yml")))
        .collect();
    paths.sort();
    for path in &paths {
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(path).expect("read dag file"))
                .expect("parse dag file");
        let Some(entries) = doc.get("cross_slot_constraints").and_then(|v| v.as_sequence())
        else {
            continue;
        };
        for entry in entries {
            let id = entry
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("cross_slot_constraints entry missing id in {path:?}"));
            declared_ids.push(id.to_string());
        }
    }
    declared_ids.sort();

    let expected: Vec<&str> = vec![
        // book_setup_dag.yaml (5)
        "book_cbus_scaffolded_requires_entities",
        "book_entities_provisioned_requires_structure_chosen",
        "book_mandates_defined_requires_trading_profiles",
        "book_parties_assigned_requires_all_cbus_have_required_roles",
        "book_ready_requires_all_cbus_validated",
        // cbu_dag.yaml (9)
        "cbu_update_pending_proof_blocks_new_subscriptions",
        "cbu_validated_requires_commercial_client_entity",
        "cbu_validated_requires_evidence_set_verified",
        "cbu_validated_requires_uboes_in_terminal_state",
        "entity_verified_required_for_ubo_role",
        "holding_active_requires_investor_active",
        "holding_suspended_cascades_from_investor",
        "investor_active_requires_kyc_approved",
        "investor_offboarded_requires_all_holdings_closed",
        // deal_dag.yaml (12)
        "agreed_rate_card_lines_are_immutable",
        "billing_period_creation_requires_active_profile",
        "billing_profile_activation_requires_agreed_rate_card",
        "deal_active_requires_active_billing_profile",
        "deal_active_requires_all_onboarding_complete",
        "deal_contracted_requires_agreed_rate_card",
        "deal_contracted_requires_bac_approved",
        "deal_contracted_requires_primary_clearance",
        "deal_offboarded_requires_all_billing_closed",
        "deal_ubo_assessment_blocked_halts_deal",
        "onboarding_request_requires_kyc_clearance",
        "rate_card_agreed_uniqueness",
        // instrument_matrix_dag.yaml (10)
        "archived_mandate_cascades_dependents",
        "cbu_archived_requires_mandate_archived",
        "cbu_suspended_implies_mandate_suspended",
        "collateral_management_active_requires_isda",
        "deactivated_chain_requires_universe_recheck",
        "decommissioned_resource_cascades_intent",
        "isda_coverage_required_for_derivative_trading",
        "mandate_active_requires_live_settlement",
        "mandate_requires_validated_cbu",
        "retired_gateway_prunes_routing_rules",
        // kyc_dag.yaml (4)
        "case_cannot_approve_with_unresolved_red_flags",
        "case_cannot_approve_with_unresolved_screening_hits",
        "case_cannot_approve_without_workstreams_complete",
        "doc_request_verified_triggers_workstream_advance",
        // lifecycle_resources_dag.yaml (2)
        "binding_live_requires_instance_active",
        "binding_pilot_requires_instance_serving",
        // onboarding_request_dag.yaml (2)
        "onboarding_request_requires_contracted_deal",
        "onboarding_request_requires_validated_cbu",
        // semos_maintenance_dag.yaml (5)
        "derivation_active_requires_active_upstream",
        "published_changeset_propagates_items",
        "published_phrase_requires_collision_clean",
        "retired_attribute_blocks_active_derivations",
        "srdef_complete_requires_no_gaps",
    ];
    let mut expected: Vec<String> = expected.into_iter().map(String::from).collect();
    expected.sort();

    assert_eq!(
        declared_ids, expected,
        "cross_slot_constraints declared-id set changed — update both this \
         list and cross_slot_census.rs's per-rule dispatch deliberately"
    );

    // Still true, deliberately: DagRegistry (the v1.3 dispatch-time index
    // GateChecker/GatePipeline read) does not index cross_slot_constraints.
    // R2 Stage 1's evaluator is a separate, standalone, report-only module
    // by design — see cross_slot_census.rs's own doc for why wiring this
    // into live dispatch is deferred, not bundled into Stage 1.
    let dag_registry_src = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("crates/dsl-runtime/src/cross_workspace/dag_registry.rs"),
    )
    .expect("read dag_registry.rs");
    assert!(
        !dag_registry_src.contains("cross_slot_constraints"),
        "DagRegistry now references cross_slot_constraints — if this is a \
         deliberate Stage 2 (or R0-convergence) change, update this test's \
         doc consciously; it is no longer describing 'zero consumers \
         anywhere', only 'not indexed by DagRegistry specifically'."
    );

    // Now true: a real, standalone, report-only evaluator exists.
    // `every_declared_cross_slot_constraint_is_evaluated` (in
    // `src/cross_slot_census.rs`'s own test module, DB-gated, `#[ignore]`)
    // is the authoritative runtime proof that all 49 dispatch to a real
    // outcome (asserts `reports.len() == declared.len()`, i.e. no id is
    // silently dropped by `evaluate_all`'s loop) — a source-text
    // containment check here would be weaker AND wrong (unhandled ids
    // fall through a wildcard match arm, so their id strings don't
    // literally appear in the source at all; that's fine, the loop still
    // calls `evaluate_one` for each one).
    let census_src = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cross_slot_census.rs"),
    )
    .expect("read cross_slot_census.rs — R2 Stage 1's evaluator should exist");
    assert!(
        census_src.contains("pub async fn evaluate_all"),
        "cross_slot_census::evaluate_all should exist as the real per-declared-id \
         dispatch entry point"
    );
    assert!(
        census_src.contains("fn every_declared_cross_slot_constraint_is_evaluated"),
        "cross_slot_census.rs should carry its own runtime proof test for \
         'no id silently dropped' — this file only re-checks the declared-id \
         inventory and the module's existence, not the dispatch-completeness \
         claim itself"
    );
}

/// KNOWN GAP #6 — `enforce_requires_states_precondition_with_mode`
/// (`src/dsl_v2/executor.rs`) derives its `SlotStateProvider` lookup key from
/// the verb's own YAML domain string (e.g. `kyc-case`, `entity-workstream`,
/// `screening`), not from `slot_state_table.yaml`'s registered workspace key
/// (`kyc`). For every hyphenated/multi-word domain this produces a key that
/// can never match a registered entry, so the FailClosed default (production)
/// refuses the verb outright with `no_slot_mapping` — REGARDLESS of the
/// entity's real state. `cbu`/`deal` verbs are unaffected only because those
/// domain strings happen to equal their own registered `workspace.workspace`
/// self-slot (`cbu.cbu`, `deal.deal`).
///
/// Execution-confirmed against the real dev DB (workstream
/// `89d8fd8e-9809-454d-9ff2-b064cd555d77` in SCREEN, case
/// `eb70b1df-c307-4da6-866a-a816351d9422` in INTAKE, real
/// `runtime_registry()`, real `slot_state_table.yaml` loaded exactly as
/// `ob-poc-web::main` loads it): `entity-workstream.update-status`,
/// `screening.run`, `kyc-case.escalate`, `kyc-case.close`, and
/// `kyc-case.refer` all refused with `no_slot_mapping` — not a business-rule
/// refusal — and all five passed unconditionally under
/// `OB_POC_LIFECYCLE_GATE_MODE=fail-open`, including `screening.run` whose
/// OWN declared `requires_states` (`[PENDING, VERIFY, workstream_open]`)
/// does not even include the entity's real state (`SCREEN`) — proving the
/// gate never reaches its own precondition logic in either mode.
///
/// This test statically re-derives the SAME lookup key
/// `enforce_requires_states_precondition_with_mode` computes (mirroring
/// executor.rs's `derived_slot`/fallback logic exactly) for every verb
/// declaring both `requires_states` and `entity_arg`, and pins the exact set
/// that fails to resolve against the real `slot_state_table.yaml`. This does
/// NOT close the gap — it pins it so the fix (and it is one fix: derive the
/// lookup key from a real workspace, not the verb's own domain string) must
/// touch this file consciously.
#[test]
fn requires_states_domain_key_resolution_gap_is_exactly_known() {
    let slot_table_raw =
        fs::read_to_string(config_root().join("slot_state_table.yaml")).expect("read slot table");
    let slot_table: std::collections::HashMap<String, Vec<String>> =
        serde_yaml::from_str(&slot_table_raw).expect("parse slot table");
    let slot_keys: HashSet<String> = slot_table.into_keys().collect();

    let mut unresolved: Vec<String> = Vec::new();
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
                let domain = domain.as_str().unwrap_or_default();
                let Some(verbs) = body.get("verbs").and_then(|v| v.as_mapping()) else {
                    continue;
                };
                for (verb, spec) in verbs {
                    let verb = verb.as_str().unwrap_or_default();
                    let Some(lifecycle) = spec.get("lifecycle") else {
                        continue;
                    };
                    let requires_states_nonempty = lifecycle
                        .get("requires_states")
                        .and_then(|r| r.as_sequence())
                        .is_some_and(|seq| !seq.is_empty());
                    let Some(entity_arg) =
                        lifecycle.get("entity_arg").and_then(|e| e.as_str())
                    else {
                        continue;
                    };
                    if !requires_states_nonempty {
                        continue;
                    }
                    // Mirrors executor.rs:2166-2173 EXACTLY: derived_slot
                    // strips a trailing "-id", kebab->snake, prefixed by the
                    // verb's own domain string; fallback tries domain.domain.
                    let derived_slot = entity_arg
                        .strip_suffix("-id")
                        .map(|stem| format!("{}_{}", domain, stem.replace('-', "_")));
                    let key1 = derived_slot.as_ref().map(|s| format!("{domain}.{s}"));
                    let key2 = format!("{domain}.{domain}");
                    let resolved = key1.as_ref().is_some_and(|k| slot_keys.contains(k))
                        || slot_keys.contains(&key2);
                    if !resolved {
                        unresolved.push(format!("{domain}.{verb}"));
                    }
                }
            }
        }
    }
    unresolved.sort();
    unresolved.dedup();

    let expected: Vec<&str> = vec![
        "application-instance.activate",
        "application-instance.bring-online",
        "application-instance.decommission",
        "application-instance.enter-maintenance",
        "application-instance.exit-maintenance",
        "application-instance.take-offline",
        "billing.activate-profile",
        "billing.approve-period",
        "billing.calculate-period",
        "billing.close-profile",
        "billing.dispute-period",
        "billing.generate-invoice",
        "billing.review-period",
        "billing.suspend-profile",
        "capability-binding.abort-pilot",
        "capability-binding.deprecate",
        "capability-binding.promote-live",
        "capability-binding.retire",
        "capability-binding.start-pilot",
        "entity-workstream.complete",
        "entity-workstream.update-status",
        "evidence.mark-verified",
        "governance.publish",
        "governance.record-review",
        "governance.rollback",
        "governance.submit-for-review",
        "kyc-case.approve",
        "kyc-case.close",
        "kyc-case.escalate",
        "kyc-case.refer",
        "kyc-case.reject",
        "kyc-case.update-status",
        "red-flag.escalate",
        "screening.bulk-refresh",
        "screening.complete",
        "screening.run",
        "service-consumption.activate",
        "service-consumption.begin-winddown",
        "service-consumption.provision",
        "service-consumption.reinstate",
        "service-consumption.retire",
        "service-consumption.suspend",
        "service-resource.activate",
        "service-resource.decommission",
        "service-resource.suspend",
        "service-version.publish",
        "service-version.retire",
        "service-version.submit-for-review",
        "service.define",
        "service.deprecate",
        "service.propose-revision",
        "service.retire",
        "settlement-chain.deactivate-chain",
        "trade-gateway.activate-gateway",
        "trade-gateway.suspend-gateway",
        "trading-profile.approve",
        "trading-profile.archive",
        // trading-profile.create-draft REMOVED from this pin (2026-08-19,
        // EOP-PLAN-GAMEBOARD-001): its `lifecycle` block was deleted
        // entirely — the generic requires_states mechanism could never
        // have worked for a verb that creates a NEW row (no existing
        // profile-id to check state against). It no longer declares
        // requires_states at all, so it's no longer in the "always
        // no_slot_mapping-refuses" gap set. Real precondition is now a
        // dedicated check, enforce_trading_profile_no_active_draft
        // (src/dsl_v2/executor.rs) — a deliberate move OUT of this pin,
        // not a regression (per this test's own doc comment).
        "trading-profile.reject",
        "trading-profile.submit",
    ];
    let mut expected: Vec<String> = expected.into_iter().map(String::from).collect();
    expected.sort();
    expected.dedup();

    assert_eq!(
        unresolved, expected,
        "requires_states domain-key resolution gap changed — this is a KNOWN \
         GAP pin (60 verbs whose gate always refuses with no_slot_mapping \
         regardless of real state), not a closure; update deliberately. A \
         verb moving OUT of this set without a corresponding \
         slot_state_table.yaml fix is a regression (it means the verb quietly \
         stopped declaring requires_states, not that the gap closed); a verb \
         moving IN means a newly-authored requires_states block is silently \
         dead on arrival."
    );
}

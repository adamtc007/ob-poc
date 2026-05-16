use serde_yaml::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn yaml_files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", dir.display());
    }) {
        let entry = entry.expect("directory entry readable");
        let path = entry.path();
        if path.is_dir() {
            yaml_files_under(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("yaml") {
            out.push(path);
        }
    }
}

fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    yaml_files_under(dir, &mut files);
    files.sort();
    files
}

fn parse_yaml(path: &Path) -> Value {
    let source = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });
    serde_yaml::from_str(&source).unwrap_or_else(|err| {
        panic!("failed to parse {}: {err}", path.display());
    })
}

fn canonical_json(value: &Value) -> String {
    fn sort_json(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(sort_json).collect())
            }
            serde_json::Value::Object(map) => {
                let mut sorted = serde_json::Map::new();
                let mut entries = map.into_iter().collect::<Vec<_>>();
                entries.sort_by(|left, right| left.0.cmp(&right.0));
                for (key, value) in entries {
                    sorted.insert(key, sort_json(value));
                }
                serde_json::Value::Object(sorted)
            }
            scalar => scalar,
        }
    }

    let json = serde_json::to_value(value).expect("yaml converts to json");
    serde_json::to_string(&sort_json(json)).expect("canonical json serializes")
}

fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.as_mapping()?.get(Value::String(key.to_string()))
}

fn as_sequence(value: Option<&Value>) -> impl Iterator<Item = &Value> {
    value.and_then(Value::as_sequence).into_iter().flatten()
}

fn string_set(value: &Value, key: &str) -> BTreeSet<String> {
    as_sequence(get(value, key))
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn find_yaml_by_field(dir: &Path, fields: &[&str], expected: &str) -> (PathBuf, Value) {
    for path in yaml_files(dir) {
        let yaml = parse_yaml(&path);
        if fields
            .iter()
            .any(|field| get(&yaml, field).and_then(Value::as_str) == Some(expected))
        {
            return (path, yaml);
        }
    }
    panic!(
        "failed to find yaml in {} with any of {fields:?} = {expected}",
        dir.display()
    );
}

fn fqn_tokens(source: &str) -> BTreeSet<String> {
    source
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_')))
        .filter(|token| {
            let Some((domain, verb)) = token.split_once('.') else {
                return false;
            };
            !domain.is_empty()
                && !verb.is_empty()
                && domain
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        })
        .map(ToString::to_string)
        .collect()
}

fn prefixed_tokens(source: &str, prefixes: &BTreeSet<String>) -> BTreeSet<String> {
    fqn_tokens(source)
        .into_iter()
        .filter(|token| prefixes.iter().any(|prefix| token.starts_with(prefix)))
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct DomainPackReload {
    pack_id: String,
    owned_dags: BTreeSet<String>,
    owned_packs: BTreeSet<String>,
    owned_state_machines: BTreeSet<String>,
    owned_constellation_maps: BTreeSet<String>,
    owned_constellation_families: BTreeSet<String>,
    owned_universes: BTreeSet<String>,
    owned_verb_prefixes: BTreeSet<String>,
    owned_entity_kinds: BTreeSet<String>,
    surface_hash: String,
    surface_payloads: BTreeMap<String, String>,
}

fn load_domain_pack_taxonomy_from_yaml(path: &Path) -> DomainPackReload {
    let root = repo_root();
    let domain_pack = parse_yaml(path);
    let pack_id = get(&domain_pack, "pack_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("domain pack {} declares pack_id", path.display()))
        .to_string();

    let owned_dags = string_set(&domain_pack, "owned_dags");
    let owned_packs = string_set(&domain_pack, "owned_packs");
    let owned_state_machines = string_set(&domain_pack, "owned_state_machines");
    let owned_constellation_maps = string_set(&domain_pack, "owned_constellation_maps");
    let owned_constellation_families = string_set(&domain_pack, "owned_constellation_families");
    let owned_universes = string_set(&domain_pack, "owned_universes");
    let owned_verb_prefixes = string_set(&domain_pack, "owned_verb_prefixes");
    let owned_entity_kinds = string_set(&domain_pack, "owned_entity_kinds");

    let mut surface_payloads = BTreeMap::new();
    surface_payloads.insert(
        format!("domain_pack:{pack_id}"),
        canonical_json(&domain_pack),
    );

    for dag_id in &owned_dags {
        let (_, yaml) = find_yaml_by_field(
            &root.join("config/sem_os_seeds/dag_taxonomies"),
            &["dag_id", "workspace"],
            dag_id,
        );
        surface_payloads.insert(format!("dag:{dag_id}"), canonical_json(&yaml));
    }

    for pack_id in &owned_packs {
        let (_, yaml) = find_yaml_by_field(&root.join("config/packs"), &["id"], pack_id);
        surface_payloads.insert(format!("pack:{pack_id}"), canonical_json(&yaml));

        let macro_definitions = macro_definitions();
        for allowed in as_sequence(get(&yaml, "allowed_verbs")).filter_map(Value::as_str) {
            if let Some((_path, macro_body)) = macro_definitions.get(allowed) {
                surface_payloads.insert(format!("macro:{allowed}"), canonical_json(macro_body));
            }
        }
    }

    for state_machine in &owned_state_machines {
        let (_, yaml) = find_yaml_by_field(
            &root.join("config/sem_os_seeds/state_machines"),
            &["state_machine"],
            state_machine,
        );
        surface_payloads.insert(
            format!("state_machine:{state_machine}"),
            canonical_json(&yaml),
        );
    }

    for constellation in &owned_constellation_maps {
        let (_, yaml) = find_yaml_by_field(
            &root.join("config/sem_os_seeds/constellation_maps"),
            &["constellation"],
            constellation,
        );
        surface_payloads.insert(
            format!("constellation_map:{constellation}"),
            canonical_json(&yaml),
        );
    }

    for family in &owned_constellation_families {
        let (_, yaml) = find_yaml_by_field(
            &root.join("config/sem_os_seeds/constellation_families"),
            &["family_id", "fqn"],
            family,
        );
        surface_payloads.insert(
            format!("constellation_family:{family}"),
            canonical_json(&yaml),
        );
    }

    for universe in &owned_universes {
        let (_, yaml) = find_yaml_by_field(
            &root.join("config/sem_os_seeds/universes"),
            &["fqn", "universe_id"],
            universe,
        );
        surface_payloads.insert(format!("universe:{universe}"), canonical_json(&yaml));
    }

    let entity_taxonomy = parse_yaml(&root.join("config/ontology/entity_taxonomy.yaml"));
    let entity_defs = get(&entity_taxonomy, "entities")
        .and_then(Value::as_mapping)
        .expect("entity taxonomy declares entities");
    for entity_kind in &owned_entity_kinds {
        assert!(
            entity_defs.contains_key(Value::String(entity_kind.clone())),
            "domain pack {pack_id} owns unknown entity kind {entity_kind}"
        );
    }
    surface_payloads.insert(
        "ontology:entity_taxonomy".to_string(),
        canonical_json(&entity_taxonomy),
    );

    let mut hasher = Sha256::new();
    for (surface, payload) in &surface_payloads {
        hasher.update(surface.as_bytes());
        hasher.update(b"\n");
        hasher.update(payload.as_bytes());
        hasher.update(b"\n");
    }

    DomainPackReload {
        pack_id,
        owned_dags,
        owned_packs,
        owned_state_machines,
        owned_constellation_maps,
        owned_constellation_families,
        owned_universes,
        owned_verb_prefixes,
        owned_entity_kinds,
        surface_hash: hex::encode(hasher.finalize()),
        surface_payloads,
    }
}

fn domain_pack_paths() -> Vec<PathBuf> {
    yaml_files(&repo_root().join("config/sem_os_seeds/domain_packs"))
}

fn registry_verbs() -> BTreeSet<String> {
    let mut verbs = BTreeSet::new();
    for path in yaml_files(&repo_root().join("config/verbs")) {
        let yaml = parse_yaml(&path);
        let Some(domains) = get(&yaml, "domains").and_then(Value::as_mapping) else {
            continue;
        };

        for (domain, spec) in domains {
            let Some(domain) = domain.as_str() else {
                continue;
            };
            let Some(domain_verbs) = get(spec, "verbs").and_then(Value::as_mapping) else {
                continue;
            };
            for verb in domain_verbs.keys().filter_map(Value::as_str) {
                verbs.insert(format!("{domain}.{verb}"));
            }
        }
    }
    verbs
}

fn macro_expansions() -> BTreeMap<String, BTreeSet<String>> {
    let mut expansions = BTreeMap::new();
    for path in yaml_files(&repo_root().join("config/verb_schemas/macros")) {
        let yaml = parse_yaml(&path);
        let Some(macros) = yaml.as_mapping() else {
            continue;
        };
        for (macro_name, spec) in macros {
            let Some(macro_name) = macro_name.as_str() else {
                continue;
            };
            let mut verbs = BTreeSet::new();
            for step in as_sequence(get(spec, "expands-to")) {
                if let Some(verb) = get(step, "verb").and_then(Value::as_str) {
                    verbs.insert(verb.to_string());
                }
            }
            if !verbs.is_empty() {
                expansions.insert(macro_name.to_string(), verbs);
            }
        }
    }
    expansions
}

fn macro_definitions() -> BTreeMap<String, (PathBuf, Value)> {
    let mut definitions = BTreeMap::new();
    for path in yaml_files(&repo_root().join("config/verb_schemas/macros")) {
        let yaml = parse_yaml(&path);
        let Some(macros) = yaml.as_mapping() else {
            continue;
        };
        for (macro_name, spec) in macros {
            let Some(macro_name) = macro_name.as_str() else {
                continue;
            };
            assert!(
                definitions
                    .insert(macro_name.to_string(), (path.clone(), spec.clone()))
                    .is_none(),
                "duplicate macro definition {macro_name}"
            );
        }
    }
    definitions
}

fn unpack_macro_atomics(
    macro_name: &str,
    expansions: &BTreeMap<String, BTreeSet<String>>,
    stack: &mut Vec<String>,
) -> BTreeSet<String> {
    if stack.iter().any(|entry| entry == macro_name) {
        panic!("macro expansion cycle: {stack:?} -> {macro_name}");
    }
    stack.push(macro_name.to_string());

    let mut atomics = BTreeSet::new();
    for verb in expansions
        .get(macro_name)
        .unwrap_or_else(|| panic!("unknown macro {macro_name}"))
    {
        if verb != macro_name && expansions.contains_key(verb) {
            atomics.extend(unpack_macro_atomics(verb, expansions, stack));
        } else {
            atomics.insert(verb.clone());
        }
    }

    stack.pop();
    atomics
}

fn owned_pack_allowed_verbs(pack_ids: &BTreeSet<String>) -> BTreeMap<String, BTreeSet<String>> {
    let mut by_pack = BTreeMap::new();
    for pack_id in pack_ids {
        let (_, yaml) = find_yaml_by_field(&repo_root().join("config/packs"), &["id"], pack_id);
        let allowed = as_sequence(get(&yaml, "allowed_verbs"))
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        by_pack.insert(pack_id.clone(), allowed);
    }
    by_pack
}

fn owned_dag_verbs(dag_ids: &BTreeSet<String>, prefixes: &BTreeSet<String>) -> BTreeSet<String> {
    let mut verbs = BTreeSet::new();
    for dag_id in dag_ids {
        let (path, _) = find_yaml_by_field(
            &repo_root().join("config/sem_os_seeds/dag_taxonomies"),
            &["dag_id", "workspace"],
            dag_id,
        );
        let source = fs::read_to_string(&path).expect("DAG source readable");
        verbs.extend(prefixed_tokens(&source, prefixes));
    }
    verbs
}

fn domain_pack_transition_verbs(path: &Path) -> BTreeSet<String> {
    let yaml = parse_yaml(path);
    as_sequence(get(&yaml, "allowed_transitions"))
        .filter_map(|transition| get(transition, "verb").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect()
}

#[test]
fn domain_pack_taxonomy_reload_from_yaml_is_idempotent() {
    let paths = domain_pack_paths();
    assert!(!paths.is_empty(), "expected domain pack manifests");

    for path in paths {
        let first = load_domain_pack_taxonomy_from_yaml(&path);
        let second = load_domain_pack_taxonomy_from_yaml(&path);

        assert_eq!(first, second, "reload was not idempotent for {path:?}");
        assert!(!first.pack_id.is_empty());
        assert!(!first.owned_packs.is_empty());
        assert!(!first.owned_dags.is_empty());
        assert!(!first.owned_verb_prefixes.is_empty());
        assert!(!first.surface_hash.is_empty());
    }
}

#[test]
fn domain_pack_owned_dsl_verbs_are_represented_in_owned_dags() {
    let registry_verbs = registry_verbs();
    let macro_expansions = macro_expansions();
    let mut failures = Vec::new();

    for path in domain_pack_paths() {
        let reload = load_domain_pack_taxonomy_from_yaml(&path);
        let dag_verbs = owned_dag_verbs(&reload.owned_dags, &reload.owned_verb_prefixes);
        let pack_allowed = owned_pack_allowed_verbs(&reload.owned_packs);

        let mut owned_dsl_verbs = BTreeSet::new();
        for allowed in pack_allowed.values().flatten() {
            if reload
                .owned_verb_prefixes
                .iter()
                .any(|prefix| allowed.starts_with(prefix))
            {
                owned_dsl_verbs.insert(allowed.clone());
            }

            if let Some(expansion) = macro_expansions.get(allowed) {
                for verb in expansion {
                    if reload
                        .owned_verb_prefixes
                        .iter()
                        .any(|prefix| verb.starts_with(prefix))
                    {
                        owned_dsl_verbs.insert(verb.clone());
                    }
                }
                for verb in unpack_macro_atomics(allowed, &macro_expansions, &mut Vec::new()) {
                    if reload
                        .owned_verb_prefixes
                        .iter()
                        .any(|prefix| verb.starts_with(prefix))
                    {
                        owned_dsl_verbs.insert(verb);
                    }
                }
            }
        }

        for verb in domain_pack_transition_verbs(&path) {
            if reload
                .owned_verb_prefixes
                .iter()
                .any(|prefix| verb.starts_with(prefix))
            {
                owned_dsl_verbs.insert(verb);
            }
        }

        for verb in &owned_dsl_verbs {
            if !registry_verbs.contains(verb) && !macro_expansions.contains_key(verb) {
                failures.push(format!(
                    "{} owns DSL verb {verb}, but it is not in registry verbs or macro definitions",
                    reload.pack_id
                ));
            }
            if !dag_verbs.contains(verb) {
                failures.push(format!(
                    "{} owns DSL verb {verb}, but no owned DAG declares it",
                    reload.pack_id
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "domain pack DSL/DAG reconciliation failures: {failures:#?}"
    );
}

#[test]
fn domain_pack_macros_do_not_hide_owned_primitives_from_pack_allowlists() {
    let macro_expansions = macro_expansions();
    let mut failures = Vec::new();

    for path in domain_pack_paths() {
        let reload = load_domain_pack_taxonomy_from_yaml(&path);
        let pack_allowed = owned_pack_allowed_verbs(&reload.owned_packs);

        for (pack_id, allowed) in pack_allowed {
            for macro_name in allowed
                .iter()
                .filter(|name| macro_expansions.contains_key(*name))
            {
                for step in &macro_expansions[macro_name] {
                    if step != macro_name
                        && macro_expansions.contains_key(step)
                        && !allowed.contains(step)
                    {
                        failures.push(format!(
                            "{} / {}: {} expands to nested macro {}",
                            reload.pack_id, pack_id, macro_name, step
                        ));
                    }
                }

                for atomic in unpack_macro_atomics(macro_name, &macro_expansions, &mut Vec::new()) {
                    if reload
                        .owned_verb_prefixes
                        .iter()
                        .any(|prefix| atomic.starts_with(prefix))
                        && !allowed.contains(&atomic)
                    {
                        failures.push(format!(
                            "{} / {}: {} expands to owned primitive {}",
                            reload.pack_id, pack_id, macro_name, atomic
                        ));
                    }
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "domain packs allow macros without their owned primitive verbs: {failures:#?}"
    );
}

#[test]
fn domain_pack_owned_macros_are_reload_surfaces() {
    let macro_definitions = macro_definitions();
    let mut failures = Vec::new();

    for path in domain_pack_paths() {
        let reload = load_domain_pack_taxonomy_from_yaml(&path);
        let pack_allowed = owned_pack_allowed_verbs(&reload.owned_packs);
        for macro_name in pack_allowed
            .values()
            .flatten()
            .filter(|allowed| macro_definitions.contains_key(*allowed))
        {
            if !reload
                .surface_payloads
                .contains_key(&format!("macro:{macro_name}"))
            {
                failures.push(format!(
                    "{} allows macro {} but reload surface omitted it",
                    reload.pack_id, macro_name
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "domain pack macro reload surface gaps: {failures:#?}"
    );
}

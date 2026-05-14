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

fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    yaml_files_under(dir, &mut files);
    files.sort();
    files
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

#[derive(Debug, PartialEq, Eq)]
struct CbuTaxonomyReload {
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

fn load_cbu_taxonomy_from_yaml() -> CbuTaxonomyReload {
    let root = repo_root();
    let domain_pack_path = root.join("config/sem_os_seeds/domain_packs/ob_poc_cbu.yaml");
    let domain_pack = parse_yaml(&domain_pack_path);
    assert_eq!(
        get(&domain_pack, "pack_id").and_then(Value::as_str),
        Some("ob-poc.cbu")
    );

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
        "domain_pack:ob-poc.cbu".to_string(),
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
            "CBU domain pack owns unknown entity kind {entity_kind}"
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
    let surface_hash = hex::encode(hasher.finalize());

    CbuTaxonomyReload {
        owned_dags,
        owned_packs,
        owned_state_machines,
        owned_constellation_maps,
        owned_constellation_families,
        owned_universes,
        owned_verb_prefixes,
        owned_entity_kinds,
        surface_hash,
        surface_payloads,
    }
}

fn cbu_tokens(source: &str) -> BTreeSet<String> {
    source
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_')))
        .filter(|token| token.starts_with("cbu.") && token.len() > "cbu.".len())
        .map(ToString::to_string)
        .collect()
}

fn cbu_registry_verbs() -> BTreeSet<String> {
    let mut files = Vec::new();
    yaml_files_under(&repo_root().join("config/verbs"), &mut files);

    let mut verbs = BTreeSet::new();
    for path in files {
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
                let fqn = format!("{domain}.{verb}");
                if fqn.starts_with("cbu.") {
                    verbs.insert(fqn);
                }
            }
        }
    }
    verbs
}

fn cbu_macro_expansions() -> BTreeMap<String, BTreeSet<String>> {
    let mut files = Vec::new();
    yaml_files_under(&repo_root().join("config/verb_schemas/macros"), &mut files);

    let mut expansions = BTreeMap::new();
    for path in files {
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
                    if verb.starts_with("cbu.") {
                        verbs.insert(verb.to_string());
                    }
                }
            }
            if !verbs.is_empty() {
                expansions.insert(macro_name.to_string(), verbs);
            }
        }
    }
    expansions
}

fn cbu_template_verbs() -> BTreeSet<String> {
    let mut files = Vec::new();
    yaml_files_under(&repo_root().join("config/verbs/templates"), &mut files);

    let mut verbs = BTreeSet::new();
    for path in files {
        let yaml = parse_yaml(&path);
        if let Some(body) = get(&yaml, "body").and_then(Value::as_str) {
            verbs.extend(cbu_tokens(body));
        }
    }
    verbs
}

fn cbu_pack_primitives_and_macro_entries() -> (BTreeSet<String>, BTreeMap<String, BTreeSet<String>>)
{
    let mut files = Vec::new();
    yaml_files_under(&repo_root().join("config/packs"), &mut files);
    let macro_expansions = cbu_macro_expansions();

    let mut primitives = BTreeSet::new();
    let mut pack_macros = BTreeMap::new();
    for path in files {
        let yaml = parse_yaml(&path);
        let pack_id = get(&yaml, "id").and_then(Value::as_str).unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("pack")
        });

        let mut macros = BTreeSet::new();
        for allowed in as_sequence(get(&yaml, "allowed_verbs")).filter_map(Value::as_str) {
            if allowed.starts_with("cbu.") {
                primitives.insert(allowed.to_string());
            }
            if macro_expansions.contains_key(allowed) {
                macros.insert(allowed.to_string());
            }
        }
        if !macros.is_empty() {
            pack_macros.insert(pack_id.to_string(), macros);
        }
    }

    (primitives, pack_macros)
}

#[test]
fn cbu_taxonomy_reload_from_yaml_is_idempotent() {
    let first = load_cbu_taxonomy_from_yaml();
    let second = load_cbu_taxonomy_from_yaml();

    assert_eq!(first, second);
    assert!(first.owned_dags.contains("cbu_dag"));
    assert!(first.owned_packs.contains("cbu-maintenance"));
    assert!(first.owned_verb_prefixes.contains("cbu."));
    assert!(first.owned_entity_kinds.contains("cbu"));
    assert!(first.surface_payloads.contains_key("dag:cbu_dag"));
}

#[test]
fn canonical_cbu_dsl_verbs_are_represented_in_cbu_dag() {
    let root = repo_root();
    let dag = fs::read_to_string(root.join("config/sem_os_seeds/dag_taxonomies/cbu_dag.yaml"))
        .expect("CBU DAG readable");
    let dag_verbs = cbu_tokens(&dag);

    let (pack_primitives, _) = cbu_pack_primitives_and_macro_entries();
    let macro_verbs = cbu_macro_expansions()
        .into_values()
        .flatten()
        .collect::<BTreeSet<_>>();

    let mut dsl_verbs = cbu_registry_verbs();
    dsl_verbs.extend(macro_verbs);
    dsl_verbs.extend(cbu_template_verbs());
    dsl_verbs.extend(pack_primitives);

    let missing = dsl_verbs
        .difference(&dag_verbs)
        .cloned()
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "CBU DSL verbs missing from cbu_dag.yaml: {missing:#?}"
    );
}

#[test]
fn packs_allow_cbu_primitives_required_by_their_macros() {
    let root = repo_root();
    let mut files = Vec::new();
    yaml_files_under(&root.join("config/packs"), &mut files);

    let macro_expansions = cbu_macro_expansions();
    let mut failures = Vec::new();

    for path in files {
        let yaml = parse_yaml(&path);
        let pack_id = get(&yaml, "id").and_then(Value::as_str).unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("pack")
        });
        let allowed = as_sequence(get(&yaml, "allowed_verbs"))
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();

        for macro_name in allowed
            .iter()
            .filter(|name| macro_expansions.contains_key(*name))
        {
            for primitive in &macro_expansions[macro_name] {
                if !allowed.contains(primitive) {
                    failures.push(format!("{pack_id}: {macro_name} expands to {primitive}"));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "packs allow CBU macros without their primitive CBU verbs: {failures:#?}"
    );
}

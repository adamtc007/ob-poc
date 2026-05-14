//! Seed scanners for pipeline-defining authored config that sits outside the
//! primitive verb YAML surface.
//!
//! These scanners deliberately preserve the authored payload shape while
//! injecting an explicit `fqn` field so the objects can participate in the
//! Sem OS registry like the existing seeded object families.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sem_os_core::seeds::{
    ConstellationFamilySeed, ConstellationMapSeed, DagTaxonomySeed, DomainPackSeed, MacroDefSeed,
    StateGraphSeed, StateMachineSeed, UniverseSeed,
};

fn repo_rust_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("sem_os_obpoc_adapter crate should live under rust/crates")
}

fn read_yaml_files(dir: &Path) -> Result<Vec<(PathBuf, serde_yaml::Value)>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
            continue;
        }
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let value = serde_yaml::from_str::<serde_yaml::Value>(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        files.push((path, value));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn yaml_to_json(value: serde_yaml::Value) -> Result<serde_json::Value> {
    serde_json::to_value(value).context("failed to convert yaml payload to json")
}

fn with_fqn(mut payload: serde_json::Value, fqn: &str) -> serde_json::Value {
    match &mut payload {
        serde_json::Value::Object(map) => {
            map.insert(
                "fqn".to_string(),
                serde_json::Value::String(fqn.to_string()),
            );
        }
        other => {
            let mut map = serde_json::Map::new();
            map.insert(
                "fqn".to_string(),
                serde_json::Value::String(fqn.to_string()),
            );
            map.insert("payload".to_string(), other.clone());
            payload = serde_json::Value::Object(map);
        }
    }
    payload
}

#[derive(Debug, Default)]
pub(crate) struct DomainPackTaxonomySeeds {
    pub(crate) macro_defs: Vec<MacroDefSeed>,
    pub(crate) universes: Vec<UniverseSeed>,
    pub(crate) constellation_families: Vec<ConstellationFamilySeed>,
    pub(crate) constellation_maps: Vec<ConstellationMapSeed>,
    pub(crate) state_machines: Vec<StateMachineSeed>,
    pub(crate) dag_taxonomies: Vec<DagTaxonomySeed>,
    pub(crate) domain_packs: Vec<DomainPackSeed>,
}

#[derive(Debug)]
struct DomainPackManifestRef {
    pack_id: String,
    owned_dags: Vec<String>,
    owned_packs: Vec<String>,
    owned_state_machines: Vec<String>,
    owned_constellation_maps: Vec<String>,
    owned_constellation_families: Vec<String>,
    owned_universes: Vec<String>,
}

fn string_vec(value: &serde_yaml::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(serde_yaml::Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(serde_yaml::Value::as_str)
        .map(ToString::to_string)
        .collect()
}

fn manifest_ref(path: &Path, value: &serde_yaml::Value) -> Result<DomainPackManifestRef> {
    let pack_id = value
        .get("pack_id")
        .and_then(serde_yaml::Value::as_str)
        .with_context(|| format!("domain pack {} does not declare pack_id", path.display()))?
        .to_string();

    Ok(DomainPackManifestRef {
        pack_id,
        owned_dags: string_vec(value, "owned_dags"),
        owned_packs: string_vec(value, "owned_packs"),
        owned_state_machines: string_vec(value, "owned_state_machines"),
        owned_constellation_maps: string_vec(value, "owned_constellation_maps"),
        owned_constellation_families: string_vec(value, "owned_constellation_families"),
        owned_universes: string_vec(value, "owned_universes"),
    })
}

fn find_yaml_by_field(
    dir: &Path,
    fields: &[&str],
    expected: &str,
) -> Result<(PathBuf, serde_yaml::Value)> {
    for (path, value) in read_yaml_files(dir)? {
        if fields
            .iter()
            .any(|field| value.get(*field).and_then(serde_yaml::Value::as_str) == Some(expected))
        {
            return Ok((path, value));
        }
    }

    anyhow::bail!(
        "failed to find yaml in {} with any of {:?} = {}",
        dir.display(),
        fields,
        expected
    )
}

fn macro_definitions() -> Result<std::collections::BTreeMap<String, (PathBuf, serde_yaml::Value)>> {
    let dir = repo_rust_root().join("config/verb_schemas/macros");
    let mut macros = std::collections::BTreeMap::new();
    for (path, value) in read_yaml_files(&dir)? {
        let Some(mapping) = value.as_mapping() else {
            continue;
        };
        for (key, body) in mapping {
            let Some(fqn) = key.as_str() else {
                continue;
            };
            if macros
                .insert(fqn.to_string(), (path.clone(), body.clone()))
                .is_some()
            {
                anyhow::bail!("duplicate macro definition {fqn}");
            }
        }
    }
    Ok(macros)
}

fn pack_allowed_verbs(config_root: &Path, pack_id: &str) -> Result<Vec<String>> {
    let (_path, pack) = find_yaml_by_field(&config_root.join("packs"), &["id"], pack_id)?;
    Ok(pack
        .get("allowed_verbs")
        .and_then(serde_yaml::Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(serde_yaml::Value::as_str)
        .map(ToString::to_string)
        .collect())
}

fn insert_unique_seed(
    seeds: &mut std::collections::BTreeMap<String, serde_json::Value>,
    fqn: String,
    payload: serde_json::Value,
    source: &Path,
) -> Result<()> {
    if let Some(existing) = seeds.get(&fqn) {
        if existing != &payload {
            anyhow::bail!(
                "conflicting payloads for Sem OS seed {} while reading {}",
                fqn,
                source.display()
            );
        }
        return Ok(());
    }
    seeds.insert(fqn, payload);
    Ok(())
}

/// Scan Sem OS taxonomy YAML through Domain Pack manifests only.
///
/// Domain packs are the ownership boundary for DAG taxonomies and Sem OS
/// taxonomy surfaces. This loader deliberately does not walk legacy taxonomy
/// directories independently; a DAG/state-machine/constellation/universe is
/// seed-visible only when a Domain Pack manifest declares ownership of it.
pub(crate) fn scan_domain_pack_taxonomy_seeds() -> Result<DomainPackTaxonomySeeds> {
    let config_root = repo_rust_root().join("config");
    let domain_pack_dir = config_root.join("sem_os_seeds/domain_packs");

    let mut domain_packs = std::collections::BTreeMap::new();
    let mut dag_taxonomies = std::collections::BTreeMap::new();
    let mut state_machines = std::collections::BTreeMap::new();
    let mut constellation_maps = std::collections::BTreeMap::new();
    let mut constellation_families = std::collections::BTreeMap::new();
    let mut universes = std::collections::BTreeMap::new();
    let mut macro_defs = std::collections::BTreeMap::new();
    let macro_definitions = macro_definitions()?;

    for (path, value) in read_yaml_files(&domain_pack_dir)? {
        let manifest = manifest_ref(&path, &value)?;
        insert_unique_seed(
            &mut domain_packs,
            manifest.pack_id.clone(),
            with_fqn(yaml_to_json(value)?, &manifest.pack_id),
            &path,
        )?;

        for pack_id in &manifest.owned_packs {
            for allowed in pack_allowed_verbs(&config_root, pack_id)? {
                let Some((path, macro_body)) = macro_definitions.get(&allowed) else {
                    continue;
                };
                insert_unique_seed(
                    &mut macro_defs,
                    allowed.clone(),
                    with_fqn(yaml_to_json(macro_body.clone())?, &allowed),
                    path,
                )?;
            }
        }

        for dag_id in manifest.owned_dags {
            let (path, yaml) = find_yaml_by_field(
                &config_root.join("sem_os_seeds/dag_taxonomies"),
                &["dag_id", "workspace"],
                &dag_id,
            )?;
            let fqn = yaml
                .get("dag_id")
                .or_else(|| yaml.get("workspace"))
                .and_then(serde_yaml::Value::as_str)
                .unwrap_or(&dag_id)
                .to_string();
            insert_unique_seed(
                &mut dag_taxonomies,
                fqn.clone(),
                with_fqn(yaml_to_json(yaml)?, &fqn),
                &path,
            )?;
        }

        for state_machine in manifest.owned_state_machines {
            let (path, yaml) = find_yaml_by_field(
                &config_root.join("sem_os_seeds/state_machines"),
                &["state_machine"],
                &state_machine,
            )?;
            let fqn = yaml
                .get("state_machine")
                .and_then(serde_yaml::Value::as_str)
                .unwrap_or(&state_machine)
                .to_string();
            insert_unique_seed(
                &mut state_machines,
                fqn.clone(),
                with_fqn(yaml_to_json(yaml)?, &fqn),
                &path,
            )?;
        }

        for constellation in manifest.owned_constellation_maps {
            let (path, yaml) = find_yaml_by_field(
                &config_root.join("sem_os_seeds/constellation_maps"),
                &["constellation"],
                &constellation,
            )?;
            let fqn = yaml
                .get("constellation")
                .and_then(serde_yaml::Value::as_str)
                .unwrap_or(&constellation)
                .to_string();
            insert_unique_seed(
                &mut constellation_maps,
                fqn.clone(),
                with_fqn(yaml_to_json(yaml)?, &fqn),
                &path,
            )?;
        }

        for family in manifest.owned_constellation_families {
            let (path, yaml) = find_yaml_by_field(
                &config_root.join("sem_os_seeds/constellation_families"),
                &["family_id", "fqn"],
                &family,
            )?;
            let fqn = yaml
                .get("fqn")
                .or_else(|| yaml.get("family_id"))
                .and_then(serde_yaml::Value::as_str)
                .unwrap_or(&family)
                .to_string();
            insert_unique_seed(
                &mut constellation_families,
                fqn.clone(),
                with_fqn(yaml_to_json(yaml)?, &fqn),
                &path,
            )?;
        }

        for universe in manifest.owned_universes {
            let (path, yaml) = find_yaml_by_field(
                &config_root.join("sem_os_seeds/universes"),
                &["fqn", "universe_id"],
                &universe,
            )?;
            let fqn = yaml
                .get("fqn")
                .or_else(|| yaml.get("universe_id"))
                .and_then(serde_yaml::Value::as_str)
                .unwrap_or(&universe)
                .to_string();
            insert_unique_seed(
                &mut universes,
                fqn.clone(),
                with_fqn(yaml_to_json(yaml)?, &fqn),
                &path,
            )?;
        }
    }

    Ok(DomainPackTaxonomySeeds {
        macro_defs: macro_defs
            .into_iter()
            .map(|(fqn, payload)| MacroDefSeed { fqn, payload })
            .collect(),
        universes: universes
            .into_iter()
            .map(|(fqn, payload)| UniverseSeed { fqn, payload })
            .collect(),
        constellation_families: constellation_families
            .into_iter()
            .map(|(fqn, payload)| ConstellationFamilySeed { fqn, payload })
            .collect(),
        constellation_maps: constellation_maps
            .into_iter()
            .map(|(fqn, payload)| ConstellationMapSeed { fqn, payload })
            .collect(),
        state_machines: state_machines
            .into_iter()
            .map(|(fqn, payload)| StateMachineSeed { fqn, payload })
            .collect(),
        dag_taxonomies: dag_taxonomies
            .into_iter()
            .map(|(fqn, payload)| DagTaxonomySeed { fqn, payload })
            .collect(),
        domain_packs: domain_packs
            .into_iter()
            .map(|(fqn, payload)| DomainPackSeed { fqn, payload })
            .collect(),
    })
}

/// Scan stategraph YAML into Sem OS `StateGraphSeed` objects.
pub(crate) fn scan_state_graphs() -> Result<Vec<StateGraphSeed>> {
    let dir = repo_rust_root().join("config/stategraphs");
    let mut seeds = Vec::new();
    for (_path, value) in read_yaml_files(&dir)? {
        let Some(graph_id) = value
            .get("graph_id")
            .and_then(serde_yaml::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let fqn = format!("stategraph.{graph_id}");
        let payload = with_fqn(yaml_to_json(value)?, &fqn);
        seeds.push(StateGraphSeed { fqn, payload });
    }
    seeds.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(seeds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_state_graphs_with_prefixed_fqns() {
        let seeds = scan_state_graphs().unwrap();
        assert!(!seeds.is_empty());
        assert!(seeds.iter().any(|seed| seed.fqn == "stategraph.entity"));
    }

    #[test]
    fn scans_sem_os_taxonomies_through_domain_pack_manifests() {
        let seeds = scan_domain_pack_taxonomy_seeds().unwrap();
        assert!(!seeds.domain_packs.is_empty());
        assert!(!seeds.macro_defs.is_empty());
        assert!(!seeds.dag_taxonomies.is_empty());
        assert!(!seeds.constellation_maps.is_empty());

        for seed in seeds
            .domain_packs
            .iter()
            .map(|seed| (&seed.fqn, &seed.payload))
            .chain(
                seeds
                    .macro_defs
                    .iter()
                    .map(|seed| (&seed.fqn, &seed.payload)),
            )
            .chain(
                seeds
                    .dag_taxonomies
                    .iter()
                    .map(|seed| (&seed.fqn, &seed.payload)),
            )
            .chain(
                seeds
                    .state_machines
                    .iter()
                    .map(|seed| (&seed.fqn, &seed.payload)),
            )
            .chain(
                seeds
                    .constellation_maps
                    .iter()
                    .map(|seed| (&seed.fqn, &seed.payload)),
            )
            .chain(
                seeds
                    .constellation_families
                    .iter()
                    .map(|seed| (&seed.fqn, &seed.payload)),
            )
            .chain(
                seeds
                    .universes
                    .iter()
                    .map(|seed| (&seed.fqn, &seed.payload)),
            )
        {
            assert_eq!(
                seed.1.get("fqn").and_then(|value| value.as_str()),
                Some(seed.0.as_str())
            );
        }
    }
}

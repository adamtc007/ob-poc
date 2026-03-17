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
    ConstellationFamilySeed, ConstellationMapSeed, MacroDefSeed, StateGraphSeed, StateMachineSeed,
    UniverseSeed,
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

/// Scan operator macro YAML into Sem OS `MacroDefSeed` objects.
pub fn scan_macro_defs() -> Result<Vec<MacroDefSeed>> {
    let dir = repo_rust_root().join("config/verb_schemas/macros");
    let mut seeds = Vec::new();
    for (path, value) in read_yaml_files(&dir)? {
        let Some(mapping) = value.as_mapping() else {
            continue;
        };
        for (key, body) in mapping {
            let Some(fqn) = key.as_str() else {
                continue;
            };
            let payload = with_fqn(yaml_to_json(body.clone())?, fqn);
            seeds.push(MacroDefSeed {
                fqn: fqn.to_string(),
                payload,
            });
        }
        let _ = path;
    }
    seeds.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(seeds)
}

/// Scan constellation map YAML into Sem OS `ConstellationMapSeed` objects.
pub fn scan_universes() -> Result<Vec<UniverseSeed>> {
    let dir = repo_rust_root().join("config/sem_os_seeds/universes");
    let mut seeds = Vec::new();
    for (_path, value) in read_yaml_files(&dir)? {
        let Some(fqn) = value
            .get("fqn")
            .and_then(serde_yaml::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let payload = with_fqn(yaml_to_json(value)?, &fqn);
        seeds.push(UniverseSeed { fqn, payload });
    }
    seeds.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(seeds)
}

/// Scan constellation family YAML into Sem OS `ConstellationFamilySeed` objects.
pub fn scan_constellation_families() -> Result<Vec<ConstellationFamilySeed>> {
    let dir = repo_rust_root().join("config/sem_os_seeds/constellation_families");
    let mut seeds = Vec::new();
    for (_path, value) in read_yaml_files(&dir)? {
        let Some(fqn) = value
            .get("fqn")
            .and_then(serde_yaml::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let payload = with_fqn(yaml_to_json(value)?, &fqn);
        seeds.push(ConstellationFamilySeed { fqn, payload });
    }
    seeds.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(seeds)
}

/// Scan constellation map YAML into Sem OS `ConstellationMapSeed` objects.
pub fn scan_constellation_maps() -> Result<Vec<ConstellationMapSeed>> {
    let dir = repo_rust_root().join("config/sem_os_seeds/constellation_maps");
    let mut seeds = Vec::new();
    for (_path, value) in read_yaml_files(&dir)? {
        let Some(fqn) = value
            .get("constellation")
            .and_then(serde_yaml::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let payload = with_fqn(yaml_to_json(value)?, &fqn);
        seeds.push(ConstellationMapSeed { fqn, payload });
    }
    seeds.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(seeds)
}

/// Scan reducer state machine YAML into Sem OS `StateMachineSeed` objects.
pub fn scan_state_machines() -> Result<Vec<StateMachineSeed>> {
    let dir = repo_rust_root().join("config/sem_os_seeds/state_machines");
    let mut seeds = Vec::new();
    for (_path, value) in read_yaml_files(&dir)? {
        let Some(name) = value
            .get("state_machine")
            .and_then(serde_yaml::Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let payload = with_fqn(yaml_to_json(value)?, &name);
        seeds.push(StateMachineSeed { fqn: name, payload });
    }
    seeds.sort_by(|left, right| left.fqn.cmp(&right.fqn));
    Ok(seeds)
}

/// Scan stategraph YAML into Sem OS `StateGraphSeed` objects.
pub fn scan_state_graphs() -> Result<Vec<StateGraphSeed>> {
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
    fn scans_macro_defs_with_fqns() {
        let seeds = scan_macro_defs().unwrap();
        assert!(!seeds.is_empty());
        assert!(seeds.iter().any(|seed| seed.fqn == "case.open"));
        assert!(seeds.iter().all(
            |seed| seed.payload.get("fqn").and_then(|value| value.as_str())
                == Some(seed.fqn.as_str())
        ));
    }

    #[test]
    fn scans_universes_with_fqns() {
        let seeds = scan_universes().unwrap();
        assert!(!seeds.is_empty());
        assert!(
            seeds
                .iter()
                .any(|seed| seed.fqn == "universe.client_lifecycle"),
            "expected sample universe seed"
        );
    }

    #[test]
    fn scans_constellation_families_with_fqns() {
        let seeds = scan_constellation_families().unwrap();
        assert!(!seeds.is_empty());
        assert!(
            seeds
                .iter()
                .any(|seed| seed.fqn == "family.fund_onboarding"),
            "expected sample constellation family seed"
        );
    }

    #[test]
    fn scans_constellation_maps_with_fqns() {
        let seeds = scan_constellation_maps().unwrap();
        assert!(!seeds.is_empty());
        assert!(seeds.iter().any(|seed| seed.fqn == "struct.ie.ucits.icav"));
    }

    #[test]
    fn scans_state_machines_with_fqns() {
        let seeds = scan_state_machines().unwrap();
        assert_eq!(seeds.len(), 3);
        assert!(seeds.iter().any(|seed| seed.fqn == "kyc_case_lifecycle"));
    }

    #[test]
    fn scans_state_graphs_with_prefixed_fqns() {
        let seeds = scan_state_graphs().unwrap();
        assert!(!seeds.is_empty());
        assert!(seeds.iter().any(|seed| seed.fqn == "stategraph.entity"));
    }
}

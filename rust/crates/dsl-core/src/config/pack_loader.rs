//! Pack YAML loader (v1.2 P.9-wiring — 2026-04-23).
//!
//! Loads pack files from `config/packs/*.yaml` and extracts the
//! `allowed_verbs` list + pack name. Feeds the V1.2-5 pack-hygiene
//! validator (see `validator::validate_pack_fqns`).
//!
//! Pack YAML shape (minimal fields consumed):
//!
//! ```yaml
//! id: catalogue          # or implied from filename
//! allowed_verbs:
//!   - domain.verb-name
//!   - ...
//! ```
//!
//! Unknown fields ignored. Scanner is tolerant to the variety of pack
//! schemas already in the repo (session-bootstrap, book-setup, etc.).

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone, Deserialize)]
struct PackYaml {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    allowed_verbs: Vec<String>,
}

/// A loaded pack — `name` + its `allowed_verbs` FQN list.
#[derive(Debug, Clone, Default)]
pub struct LoadedPack {
    pub name: String,
    pub source_path: PathBuf,
    pub allowed_verbs: Vec<String>,
}

/// Load every `*.yaml` file in the packs directory. Returns a map keyed
/// by pack name. Malformed packs are skipped with a tracing warning
/// rather than failing the whole load — matches the rollout-tolerant
/// philosophy of the catalogue-load gate.
pub fn load_packs_from_dir(packs_dir: &Path) -> Result<BTreeMap<String, LoadedPack>> {
    let mut out = BTreeMap::new();
    let entries =
        fs::read_dir(packs_dir).with_context(|| format!("cannot read packs dir {packs_dir:?}"))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(?path, "pack load: {e}");
                continue;
            }
        };
        let parsed: Result<PackYaml, _> = serde_yaml::from_str(&raw);
        let pack = match parsed {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(?path, "pack parse: {e}");
                continue;
            }
        };
        let name = pack.id.unwrap_or(file_stem);
        out.insert(
            name.clone(),
            LoadedPack {
                name,
                source_path: path,
                allowed_verbs: pack.allowed_verbs,
            },
        );
    }
    Ok(out)
}

/// Flatten a packs map into the iterator form validate_pack_fqns expects:
/// a sequence of `(pack_name, fqn)` tuples.
pub fn flatten_pack_entries(
    packs: &BTreeMap<String, LoadedPack>,
) -> impl Iterator<Item = (String, String)> + '_ {
    packs.values().flat_map(|p| {
        p.allowed_verbs
            .iter()
            .map(move |fqn| (p.name.clone(), fqn.clone()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn loads_packs_from_directory() {
        let dir = TempDir::new().unwrap();
        let packs_dir = dir.path();

        let p1 = packs_dir.join("foo.yaml");
        let mut f = fs::File::create(&p1).unwrap();
        writeln!(f, "id: foo\nallowed_verbs:\n  - a.one\n  - a.two\n").unwrap();

        let p2 = packs_dir.join("bar.yaml");
        let mut f = fs::File::create(&p2).unwrap();
        writeln!(f, "allowed_verbs:\n  - b.three\n").unwrap();

        let packs = load_packs_from_dir(packs_dir).unwrap();
        assert_eq!(packs.len(), 2);
        assert_eq!(packs["foo"].allowed_verbs, vec!["a.one", "a.two"]);
        // Bar had no `id`, falls back to filename stem.
        assert_eq!(packs["bar"].allowed_verbs, vec!["b.three"]);
    }

    #[test]
    fn flatten_yields_pack_entries_in_iter_form() {
        let dir = TempDir::new().unwrap();
        let p1 = dir.path().join("p.yaml");
        let mut f = fs::File::create(&p1).unwrap();
        writeln!(f, "id: p\nallowed_verbs:\n  - x.1\n  - x.2\n  - x.3\n").unwrap();
        let packs = load_packs_from_dir(dir.path()).unwrap();
        let entries: Vec<_> = flatten_pack_entries(&packs).collect();
        assert_eq!(entries.len(), 3);
        assert!(entries.contains(&("p".to_string(), "x.1".to_string())));
        assert!(entries.contains(&("p".to_string(), "x.3".to_string())));
    }

    #[test]
    fn malformed_pack_is_skipped_not_fatal() {
        let dir = TempDir::new().unwrap();
        // Invalid YAML: unbalanced brackets
        let p1 = dir.path().join("broken.yaml");
        let mut f = fs::File::create(&p1).unwrap();
        writeln!(f, "id: [unclosed").unwrap();
        let p2 = dir.path().join("good.yaml");
        let mut f = fs::File::create(&p2).unwrap();
        writeln!(f, "id: good\nallowed_verbs:\n  - g.a\n").unwrap();
        let packs = load_packs_from_dir(dir.path()).unwrap();
        // Broken one is skipped with a tracing warn; good one loads.
        assert!(packs.contains_key("good"));
        assert!(!packs.contains_key("broken"));
    }
}

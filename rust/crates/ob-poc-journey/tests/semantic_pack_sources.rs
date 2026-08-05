use std::{collections::BTreeMap, fs, path::Path};

use ob_poc_journey::pack::{load_pack_from_file, load_semantic_pack_from_file};
use semantic_pack::{ConfigValue, DependencySource};
use serde::Deserialize;

#[derive(Deserialize)]
struct LockFile {
    lock_schema_version: u32,
    packs: Vec<LockEntry>,
}

#[derive(Deserialize)]
struct LockEntry {
    source: String,
    source_sha256: String,
    pack_id: String,
    pack_version: String,
    schema_version: u32,
    canonicalization_version: u32,
    compiler_version: String,
    dependencies: Vec<DependencySource>,
    adapter_bindings: Vec<String>,
    artifact_sha256: String,
}

#[test]
fn every_journey_manifest_has_one_admitted_semantic_pack_without_drift() {
    let config = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config");
    let journey_dir = config.join("packs");
    let semantic_dir = config.join("semantic-packs");
    let mut semantic_paths = fs::read_dir(&semantic_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "yaml")
        })
        .collect::<Vec<_>>();
    semantic_paths.sort();
    assert_eq!(semantic_paths.len(), 14);

    for semantic_path in semantic_paths {
        let journey_path = journey_dir.join(semantic_path.file_name().unwrap());
        let (journey, _) = load_pack_from_file(&journey_path).unwrap();
        let semantic = load_semantic_pack_from_file(&semantic_path).unwrap();
        assert_eq!(
            semantic.identity().id.as_str(),
            format!("ob-poc.journey.{}", journey.id)
        );
        assert_eq!(semantic.capabilities().len(), 1);
        let capability = semantic.capabilities().next().unwrap();
        assert_eq!(capability.id.as_str(), format!("journey.{}", journey.id));
        assert_eq!(
            capability.adapter_binding.as_str(),
            format!("ob-poc.journey.{}", journey.id)
        );

        let phrase_texts = capability
            .phrases
            .iter()
            .map(|phrase| phrase.text.as_str())
            .collect::<Vec<_>>();
        let mut journey_phrases = journey
            .invocation_phrases
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        journey_phrases.sort_unstable();
        assert_eq!(phrase_texts, journey_phrases);
        assert_eq!(
            extension_text_list(&capability.extensions, "ob-poc.allowed_verbs"),
            journey.allowed_verbs
        );
        assert_eq!(
            extension_text_list(&capability.extensions, "ob-poc.forbidden_verbs"),
            journey.forbidden_verbs
        );
    }
}

#[test]
fn checked_in_semantic_pack_receipts_match_compilation() {
    let config = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config");
    let lock: LockFile =
        serde_yaml::from_slice(&fs::read(config.join("semantic-packs.lock")).unwrap()).unwrap();
    assert_eq!(lock.lock_schema_version, 1);
    assert_eq!(lock.packs.len(), 14);

    for expected in lock.packs {
        let pack = load_semantic_pack_from_file(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join(&expected.source),
        )
        .unwrap();
        let receipt = pack.receipt();
        assert_eq!(receipt.source_hash.as_str(), expected.source_sha256);
        assert_eq!(receipt.identity.id.as_str(), expected.pack_id);
        assert_eq!(receipt.identity.version.as_str(), expected.pack_version);
        assert_eq!(receipt.schema_version, expected.schema_version);
        assert_eq!(
            receipt.canonicalization_version,
            expected.canonicalization_version
        );
        assert_eq!(receipt.compiler_version, expected.compiler_version);
        assert_eq!(receipt.dependencies, expected.dependencies);
        assert_eq!(receipt.adapter_bindings, expected.adapter_bindings);
        assert_eq!(receipt.artifact_hash.as_str(), expected.artifact_sha256);
    }
}

fn extension_text_list(extensions: &BTreeMap<String, ConfigValue>, key: &str) -> Vec<String> {
    match extensions.get(key) {
        Some(ConfigValue::List(values)) => values
            .iter()
            .map(|value| match value {
                ConfigValue::Text(text) => text.clone(),
                other => panic!("{key} contains non-text value {other:?}"),
            })
            .collect(),
        other => panic!("missing list extension {key}: {other:?}"),
    }
}

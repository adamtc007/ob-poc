use std::{env, fs, path::PathBuf};

use ob_poc_journey::pack::load_semantic_pack_from_file;
use semantic_pack::DependencySource;
use serde::Serialize;

#[derive(Serialize)]
struct LockFile {
    lock_schema_version: u32,
    packs: Vec<LockEntry>,
}

#[derive(Serialize)]
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = env::args_os().nth(1).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/semantic-packs"),
        PathBuf::from,
    );
    let mut paths = fs::read_dir(&directory)?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "yaml")
    });
    paths.sort();

    let mut packs = Vec::with_capacity(paths.len());
    for path in paths {
        let pack = load_semantic_pack_from_file(&path)?;
        let receipt = pack.receipt();
        packs.push(LockEntry {
            source: pack.provenance().source.clone(),
            source_sha256: receipt.source_hash.as_str().to_owned(),
            pack_id: receipt.identity.id.to_string(),
            pack_version: receipt.identity.version.to_string(),
            schema_version: receipt.schema_version,
            canonicalization_version: receipt.canonicalization_version,
            compiler_version: receipt.compiler_version.clone(),
            dependencies: receipt.dependencies.clone(),
            adapter_bindings: receipt.adapter_bindings.clone(),
            artifact_sha256: receipt.artifact_hash.as_str().to_owned(),
        });
    }
    print!(
        "{}",
        serde_yaml::to_string(&LockFile {
            lock_schema_version: 1,
            packs,
        })?
    );
    Ok(())
}

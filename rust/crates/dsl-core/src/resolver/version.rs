use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionHash(pub String);

impl std::fmt::Display for VersionHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn compute_version_hash(paths: &[&Path], shape: &str, workspace: &str) -> VersionHash {
    let mut hasher = Sha256::new();
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    hasher.update(workspace.as_bytes());
    hasher.update(shape.as_bytes());

    let mut sorted = paths.iter().collect::<Vec<_>>();
    sorted.sort_by(|a, b| {
        a.to_str()
            .expect("seed path must be valid UTF-8")
            .cmp(b.to_str().expect("seed path must be valid UTF-8"))
    });
    for path in sorted {
        hasher.update(
            path.to_str()
                .expect("seed path must be valid UTF-8")
                .as_bytes(),
        );
        if let Ok(bytes) = std::fs::read(path) {
            hasher.update(bytes);
        }
    }

    VersionHash(format!("0x{}", hex::encode(hasher.finalize())))
}

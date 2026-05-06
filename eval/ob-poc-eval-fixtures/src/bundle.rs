use serde::{Deserialize, Serialize};

/// Metadata stored with every seed bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedBundleManifest {
    pub bundle_id: String,
    pub schema_version: u32,
    pub configuration_version: String,
    pub domain_pack_id: String,
    pub domain_pack_version: String,
    pub state_snapshot_id: String,
    pub checksum: Option<String>,
    pub frozen: bool,
    pub notes: Option<String>,
}

/// Parse a seed-bundle manifest from YAML.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::parse_seed_bundle_manifest_yaml;
///
/// let yaml = r#"
/// bundle_id: seed-kyc-baseline-cbu-portfolio
/// schema_version: 1
/// configuration_version: semos-v0.4.2
/// domain_pack_id: ob-poc-kyc
/// domain_pack_version: 0.1.0
/// state_snapshot_id: snap-2026-05-06-baseline
/// checksum: null
/// frozen: false
/// notes: null
/// "#;
///
/// let manifest = parse_seed_bundle_manifest_yaml(yaml).expect("manifest should parse");
/// assert_eq!(manifest.bundle_id, "seed-kyc-baseline-cbu-portfolio");
/// ```
pub fn parse_seed_bundle_manifest_yaml(
    input: &str,
) -> Result<SeedBundleManifest, serde_yaml::Error> {
    serde_yaml::from_str(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_bundle_manifest_deserializes() {
        let yaml = r#"
bundle_id: seed-kyc-baseline-cbu-portfolio
schema_version: 1
configuration_version: semos-v0.4.2
domain_pack_id: ob-poc-kyc
domain_pack_version: 0.1.0
state_snapshot_id: snap-2026-05-06-baseline
checksum: null
frozen: false
notes: null
"#;

        let manifest = parse_seed_bundle_manifest_yaml(yaml).expect("manifest should parse");

        assert_eq!(manifest.bundle_id, "seed-kyc-baseline-cbu-portfolio");
        assert!(!manifest.frozen);
    }
}

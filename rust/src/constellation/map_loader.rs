use sha2::{Digest, Sha256};

use super::error::{ConstellationError, ConstellationResult};
use super::map_def::ConstellationMapDef;
use super::validate::validate_constellation_map;

/// Load and validate a constellation map from YAML.
///
/// # Examples
/// ```rust
/// use ob_poc::constellation::load_constellation_map;
///
/// let yaml = r#"
/// constellation: demo
/// jurisdiction: LU
/// slots:
///   cbu:
///     type: cbu
///     table: cbus
///     pk: cbu_id
///     cardinality: root
/// "#;
/// let map = load_constellation_map(yaml).unwrap();
/// assert_eq!(map.constellation, "demo");
/// ```
pub fn load_constellation_map(
    yaml: &str,
) -> ConstellationResult<super::validate::ValidatedConstellationMap> {
    let definition: ConstellationMapDef =
        serde_yaml::from_str(yaml).map_err(|err| ConstellationError::Other(err.into()))?;
    let mut validated = validate_constellation_map(&definition)?;
    validated.map_revision = compute_map_revision(yaml);
    Ok(validated)
}

/// Compute the stable map revision for a YAML definition.
///
/// # Examples
/// ```rust
/// use ob_poc::constellation::compute_map_revision;
///
/// assert_eq!(compute_map_revision("demo").len(), 16);
/// ```
pub fn compute_map_revision(yaml: &str) -> String {
    let hash = Sha256::digest(yaml.as_bytes());
    hex::encode(&hash[..8])
}

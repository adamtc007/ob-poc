use super::error::{ConstellationError, ConstellationResult};
use super::map_loader::load_constellation_map;
use super::validate::ValidatedConstellationMap;

/// Load one of the baked-in constellation maps.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::constellation::load_builtin_constellation_map;
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// assert_eq!(map.constellation, "struct.lux.ucits.sicav");
/// ```
pub fn load_builtin_constellation_map(
    name: &str,
) -> ConstellationResult<ValidatedConstellationMap> {
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-')
    {
        return Err(ConstellationError::Validation(format!(
            "invalid built-in constellation map '{}'",
            name
        )));
    }

    let filename = format!("{}.yaml", name.replace(['.', '-'], "_"));
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config/sem_os_seeds/constellation_maps")
        .join(filename);
    let yaml = std::fs::read_to_string(&path).map_err(|_| {
        ConstellationError::Validation(format!("unknown built-in constellation map '{}'", name))
    })?;
    load_constellation_map(&yaml)
}

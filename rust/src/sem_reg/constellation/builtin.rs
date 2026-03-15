use super::error::{ConstellationError, ConstellationResult};
use super::map_loader::load_constellation_map;
use super::validate::ValidatedConstellationMap;

const LUX_UCITS_SICAV_YAML: &str =
    include_str!("../../../config/sem_os_seeds/constellation_maps/struct_lux_ucits_sicav.yaml");

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
    match name {
        "struct.lux.ucits.sicav" => load_constellation_map(LUX_UCITS_SICAV_YAML),
        other => Err(ConstellationError::Validation(format!(
            "unknown built-in constellation map '{}'",
            other
        ))),
    }
}

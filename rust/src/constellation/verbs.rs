use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::builtin::load_builtin_constellation_map;
use super::hydrated::HydratedConstellation;
use super::hydration::{hydrate_constellation, hydrate_constellation_summary};
use super::summary::ConstellationSummary;

/// Hydrate a constellation by built-in map name.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::constellation::handle_constellation_hydrate;
///
/// let _ = handle_constellation_hydrate(
///     pool,
///     Uuid::new_v4(),
///     None,
///     "struct.lux.ucits.sicav",
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_constellation_hydrate(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map_name: &str,
) -> Result<HydratedConstellation> {
    let map =
        load_builtin_constellation_map(map_name).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    hydrate_constellation(pool, cbu_id, case_id, &map)
        .await
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

/// Compute a summary for a built-in constellation.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::constellation::handle_constellation_summary;
///
/// let _ = handle_constellation_summary(
///     pool,
///     Uuid::new_v4(),
///     None,
///     "struct.lux.ucits.sicav",
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn handle_constellation_summary(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map_name: &str,
) -> Result<ConstellationSummary> {
    let map =
        load_builtin_constellation_map(map_name).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    hydrate_constellation_summary(pool, cbu_id, case_id, &map)
        .await
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

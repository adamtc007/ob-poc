//! Shared AffinityGraph loader with lightweight process-local caching.
//!
//! The cache key is the latest active snapshot `created_at` epoch second.

use anyhow::Result;

#[cfg(feature = "database")]
use {
    crate::sem_reg::types::{pg_rows_to_snapshot_rows, PgSnapshotRow},
    sem_os_core::affinity::AffinityGraph,
    std::sync::OnceLock,
    tokio::sync::RwLock,
};

#[cfg(feature = "database")]
type GraphCache = Option<(i64, AffinityGraph)>;

#[cfg(feature = "database")]
fn cache_cell() -> &'static RwLock<GraphCache> {
    static CACHE: OnceLock<RwLock<GraphCache>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

/// Load the active AffinityGraph with process-local caching.
#[cfg(feature = "database")]
pub async fn load_affinity_graph_cached(pool: &sqlx::PgPool) -> Result<AffinityGraph> {
    let latest_epoch = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT COALESCE(MAX(EXTRACT(EPOCH FROM created_at)::bigint), 0) \
         FROM sem_reg.snapshots WHERE status = 'active'",
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(0);

    {
        let guard = cache_cell().read().await;
        if let Some((cached_epoch, graph)) = &*guard {
            if *cached_epoch == latest_epoch {
                return Ok(graph.clone());
            }
        }
    }

    let pg_rows = sqlx::query_as::<_, PgSnapshotRow>(
        "SELECT snapshot_id, snapshot_set_id, object_type::text AS object_type, object_id, \
         version_major, version_minor, status::text AS status, \
         governance_tier::text AS governance_tier, trust_class::text AS trust_class, \
         security_label, effective_from, effective_until, predecessor_id, \
         change_type::text AS change_type, change_rationale, created_by, approved_by, definition, \
         created_at \
         FROM sem_reg.snapshots WHERE status = 'active'",
    )
    .fetch_all(pool)
    .await?;
    let snapshots = pg_rows_to_snapshot_rows(pg_rows)?;
    let graph = AffinityGraph::build(&snapshots);

    let mut guard = cache_cell().write().await;
    *guard = Some((latest_epoch, graph.clone()));
    Ok(graph)
}

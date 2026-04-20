//! Shared AffinityGraph loader with lightweight process-local caching.
//!
//! The cache key is the latest active snapshot `created_at` epoch second.
//!
//! `PgSnapshotRow` is inlined as a private type here (Phase 5a slice #24
//! relocation): the original lived in `ob_poc::sem_reg::types`, but
//! that whole module is too big to relocate alongside, and the Postgres
//! row decode is the only sem_reg surface this file touches. The
//! conversion to `sem_os_core::types::SnapshotRow` (which IS in a
//! boundary crate) is also inlined.

use anyhow::{anyhow, Result};
use std::sync::OnceLock;
use tokio::sync::RwLock;

use chrono::{DateTime, Utc};
use sem_os_core::affinity::AffinityGraph;
use sem_os_core::types::{
    ChangeType, GovernanceTier, ObjectType, SnapshotRow, SnapshotStatus, TrustClass,
};
use uuid::Uuid;

type GraphCache = Option<(i64, AffinityGraph)>;

fn cache_cell() -> &'static RwLock<GraphCache> {
    static CACHE: OnceLock<RwLock<GraphCache>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

/// Postgres row shape for `sem_reg.snapshots`. All enum columns decoded
/// as `String` to avoid `sqlx::Type` derives on the canonical enums in
/// `sem_os_core` (which is sqlx-free).
#[derive(Debug, Clone, sqlx::FromRow)]
struct PgSnapshotRow {
    snapshot_id: Uuid,
    snapshot_set_id: Option<Uuid>,
    #[sqlx(try_from = "String")]
    object_type: String,
    object_id: Uuid,
    version_major: i32,
    version_minor: i32,
    #[sqlx(try_from = "String")]
    status: String,
    #[sqlx(try_from = "String")]
    governance_tier: String,
    #[sqlx(try_from = "String")]
    trust_class: String,
    security_label: serde_json::Value,
    effective_from: DateTime<Utc>,
    effective_until: Option<DateTime<Utc>>,
    predecessor_id: Option<Uuid>,
    #[sqlx(try_from = "String")]
    change_type: String,
    change_rationale: Option<String>,
    created_by: String,
    approved_by: Option<String>,
    definition: serde_json::Value,
    created_at: DateTime<Utc>,
}

impl TryFrom<PgSnapshotRow> for SnapshotRow {
    type Error = anyhow::Error;

    fn try_from(row: PgSnapshotRow) -> Result<Self> {
        Ok(SnapshotRow {
            snapshot_id: row.snapshot_id,
            snapshot_set_id: row.snapshot_set_id,
            object_type: row
                .object_type
                .parse::<ObjectType>()
                .map_err(|_| anyhow!("invalid object_type: {}", row.object_type))?,
            object_id: row.object_id,
            version_major: row.version_major,
            version_minor: row.version_minor,
            status: row
                .status
                .parse::<SnapshotStatus>()
                .map_err(|_| anyhow!("invalid status: {}", row.status))?,
            governance_tier: row
                .governance_tier
                .parse::<GovernanceTier>()
                .map_err(|_| anyhow!("invalid governance_tier: {}", row.governance_tier))?,
            trust_class: row
                .trust_class
                .parse::<TrustClass>()
                .map_err(|_| anyhow!("invalid trust_class: {}", row.trust_class))?,
            security_label: row.security_label,
            effective_from: row.effective_from,
            effective_until: row.effective_until,
            predecessor_id: row.predecessor_id,
            change_type: row
                .change_type
                .parse::<ChangeType>()
                .map_err(|_| anyhow!("invalid change_type: {}", row.change_type))?,
            change_rationale: row.change_rationale,
            created_by: row.created_by,
            approved_by: row.approved_by,
            definition: row.definition,
            created_at: row.created_at,
        })
    }
}

/// Load the active AffinityGraph with process-local caching.
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
    let snapshots: Vec<SnapshotRow> = pg_rows
        .into_iter()
        .map(SnapshotRow::try_from)
        .collect::<Result<Vec<_>>>()?;
    let graph = AffinityGraph::build(&snapshots);

    let mut guard = cache_cell().write().await;
    *guard = Some((latest_epoch, graph.clone()));
    Ok(graph)
}

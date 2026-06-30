//! Control-edge projection — a DISPOSABLE fold of the verb stream (§5).
//!
//! The verb stream is the system of record; this projection is a materialized
//! fold (K-34). The projector is the **only** writer of these rows.

use sqlx::PgConnection;

use ob_poc_kyc_substrate::{fold_control_versioned, FoldRegistry, IntentEvent, SubjectId};

use crate::error::StoreError;
use crate::store::PgKycEventStore;

/// Outcome of a projection rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectionStats {
    /// Number of edge rows written (== the active+superseded edge count in the fold).
    pub edges_written: usize,
}

/// Writes the control-edge projection by folding the stream. Stateless.
pub struct PgKycProjector;

impl PgKycProjector {
    /// Rebuild the control-edge projection for `subject` by folding the **whole**
    /// stream (§5 `rebuild_projection`).
    ///
    /// Idempotent + convergent **by construction**: the projection is a full
    /// replace of the subject's rows from a deterministic fold, so re-running —
    /// or draining events in any order, at-least-once — yields identical rows.
    /// The projection is disposable (K-34): dropping the rows loses nothing,
    /// since the stream is the system of record.
    ///
    /// The DELETE + re-INSERT runs in the caller's transaction, so a concurrent
    /// reader never sees a half-rebuilt projection.
    pub async fn rebuild_control_edges(
        conn: &mut PgConnection,
        registry: &FoldRegistry,
        subject_root: SubjectId,
    ) -> Result<ProjectionStats, StoreError> {
        let events = PgKycEventStore::load_events(conn, subject_root).await?;
        let refs: Vec<&IntentEvent> = events.iter().collect();
        let state = fold_control_versioned(&refs, registry)?;

        sqlx::query(r#"DELETE FROM "ob-poc".kyc_control_edge_projection WHERE subject_root = $1"#)
            .bind(subject_root.0)
            .execute(&mut *conn)
            .await?;

        for edge in state.edges.values() {
            let kind = serde_json::to_value(&edge.kind)
                .map_err(|e| StoreError::Db(sqlx::Error::Encode(Box::new(e))))?;
            sqlx::query(
                r#"INSERT INTO "ob-poc".kyc_control_edge_projection
                    (subject_root, edge_id, edge_kind, from_entity_id, to_entity_id,
                     percentage, status, evidence_event_id, originating_event_id)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)"#,
            )
            .bind(subject_root.0)
            .bind(edge.id.0)
            .bind(kind)
            .bind(edge.from.0)
            .bind(edge.to.0)
            .bind(edge.percentage)
            .bind(edge.status.to_string())
            .bind(edge.evidence_event_id.map(|e| e.0))
            .bind(edge.originating_event_id.0)
            .execute(&mut *conn)
            .await?;
        }

        Ok(ProjectionStats { edges_written: state.edges.len() })
    }
}

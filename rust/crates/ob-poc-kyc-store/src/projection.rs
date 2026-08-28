//! Control-edge projection — a DISPOSABLE fold of the verb stream (§5).
//!
//! The verb stream is the system of record; this projection is a materialized
//! fold (K-34). The projector is the **only** writer of these rows.
//!
//! **No queue (2026-08-22 ruling).** These projectors are called directly, on
//! demand. There is no outbox effect-kind, no fan-out from `append`, and no
//! drainer. Events are immutable, so replay from any point yields the same
//! fold — a snapshot rebuilt on demand from an immutable stream is always
//! correct, and a queue telling you WHEN to rebuild adds nothing but a place
//! to jam. It did jam: the drainers hard-errored on any event whose lexicon
//! hash was absent from the `FoldRegistry`, and a failed drain left the row
//! `pending` so the next claim took the same row — every manifest bump
//! stranded the whole pre-bump backlog behind a head-of-line poison pill.
//! Removing the trigger dissolves that failure mode by construction.

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
                     percentage, status, originating_event_id)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
            )
            .bind(subject_root.0)
            .bind(edge.id.0)
            .bind(kind)
            .bind(edge.from.0)
            .bind(edge.to.0)
            .bind(edge.percentage)
            .bind(edge.status.to_string())
            .bind(edge.originating_event_id.0)
            .execute(&mut *conn)
            .await?;
        }

        Ok(ProjectionStats {
            edges_written: state.edges.len(),
        })
    }
}

// `PgKycObligationProjector`/`ObligationProjectionStats` REMOVED
// (EOP-DD-KYCUBO-D2.0 §5, 2026-08-22): "the outbox removal (2026-08-22)
// deleted the queue and drainers but left the obligation projection
// standing... it goes with obligations." `kyc_ubo.assert.obligation.creation`
// — the only writer of a new `ObligationTracks` entry — is dissolved, so
// the tables the projector wrote could never again receive a row.
// `kyc_obligation_projection`/`kyc_subject_rollup_projection` themselves
// were DROPPED (EOP-DD-KYCUBO-D2.1 Tranche A, 2026-08-23, migration
// `20260823_drop_kyc_obligation_projection_tables.sql`) — the projector's
// removal left them standing, frozen and readable as if current, for a
// full tranche before the tables followed it.
// Reintroduction path: none named — the run book (D2.0 §4) replaces this
// capability, not a future projector.

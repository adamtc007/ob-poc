//! Control-edge projection — a DISPOSABLE fold of the verb stream (§5).
//!
//! The verb stream is the system of record; this projection is a materialized
//! fold (K-34). The projector is the **only** writer of these rows.

use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    fold_control_versioned, fold_obligations_versioned, FoldRegistry, IntentEvent, SubjectId,
    SubjectOverallState,
};

use crate::error::StoreError;
use crate::store::PgKycEventStore;

/// Outbox effect-kind for the control-edge projection.
pub const CONTROL_EDGE_PROJECTION_EFFECT: &str = "kyc.projection.control_edges";

/// Outbox effect-kind for the obligation-graph projection (W6).
pub const OBLIGATION_PROJECTION_EFFECT: &str = "kyc.projection.obligations";

/// Every projection effect-kind `append` fans out to. Each kind has its own
/// drainer so there is no multi-consumer contention.
pub const PROJECTION_EFFECT_KINDS: &[&str] =
    &[CONTROL_EDGE_PROJECTION_EFFECT, OBLIGATION_PROJECTION_EFFECT];

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

        Ok(ProjectionStats {
            edges_written: state.edges.len(),
        })
    }
}

/// Self-contained outbox drainer for the control-edge projection.
///
/// Reuses the shared `outbox` table (the §5 drainer-reuses-the-outbox rule) but
/// not the app-level `OutboxDrainerImpl` — the reuse-via-consumer integration is
/// a cutover-time concern. Claims only [`CONTROL_EDGE_PROJECTION_EFFECT`] rows,
/// so it never contends with other consumers.
pub struct PgKycProjectionDrainer;

impl PgKycProjectionDrainer {
    /// Claim and process ONE pending control-edge projection effect, in a single
    /// transaction: `FOR UPDATE SKIP LOCKED` claim → rebuild the subject's
    /// projection → mark the row `done` → commit.
    ///
    /// At-least-once by construction: on any error the transaction rolls back and
    /// the row stays `pending` (the convergent full-rebuild projector makes
    /// reprocessing safe). The claimed row is row-locked (not a separate
    /// `processing` state) for the rebuild's duration, so a crash needs no reaper.
    /// Concurrent drainers `SKIP LOCKED` past each other's claims.
    ///
    /// Returns the re-projected subject, or `None` when the queue is empty.
    pub async fn drain_once(
        pool: &PgPool,
        registry: &FoldRegistry,
    ) -> Result<Option<SubjectId>, StoreError> {
        let mut tx = pool.begin().await?;
        let claimed = sqlx::query(
            r#"SELECT id, payload FROM "public".outbox
               WHERE effect_kind = $1 AND status = 'pending'
               ORDER BY created_at
               FOR UPDATE SKIP LOCKED
               LIMIT 1"#,
        )
        .bind(CONTROL_EDGE_PROJECTION_EFFECT)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = claimed else {
            tx.rollback().await?;
            return Ok(None);
        };

        let id: Uuid = row.get("id");
        let payload: serde_json::Value = row.get("payload");
        let subject = payload
            .get("subject_root")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(SubjectId)
            .ok_or_else(|| {
                StoreError::Db(sqlx::Error::Decode(
                    "outbox payload missing/invalid subject_root".into(),
                ))
            })?;

        PgKycProjector::rebuild_control_edges(&mut tx, registry, subject).await?;

        sqlx::query(
            r#"UPDATE "public".outbox SET status = 'done', processed_at = now() WHERE id = $1"#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(Some(subject))
    }

    /// Drain up to `max` rows, one transaction each. Returns the count processed
    /// (stops early when the queue drains).
    pub async fn drain_all(
        pool: &PgPool,
        registry: &FoldRegistry,
        max: usize,
    ) -> Result<usize, StoreError> {
        let mut processed = 0;
        while processed < max {
            if Self::drain_once(pool, registry).await?.is_none() {
                break;
            }
            processed += 1;
        }
        Ok(processed)
    }
}

// ── W6: Obligation-graph projection ──────────────────────────────────────────

/// Rebuild stats for the obligation projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObligationProjectionStats {
    pub obligations_written: usize,
    pub subjects_written: usize,
}

/// Rebuilds the obligation-graph projection for a subject by folding the stream.
/// Idempotent + convergent by construction (full replace from a deterministic fold).
pub struct PgKycObligationProjector;

impl PgKycObligationProjector {
    pub async fn rebuild_obligations(
        conn: &mut PgConnection,
        registry: &FoldRegistry,
        subject_root: SubjectId,
    ) -> Result<ObligationProjectionStats, StoreError> {
        let events = PgKycEventStore::load_events(conn, subject_root).await?;
        let refs: Vec<&IntentEvent> = events.iter().collect();
        let state = fold_obligations_versioned(&refs, registry)?;

        // Full replace (idempotent).
        sqlx::query(r#"DELETE FROM "ob-poc".kyc_obligation_projection WHERE subject_root = $1"#)
            .bind(subject_root.0)
            .execute(&mut *conn)
            .await?;
        sqlx::query(
            r#"DELETE FROM "ob-poc".kyc_subject_rollup_projection WHERE subject_root = $1"#,
        )
        .bind(subject_root.0)
        .execute(&mut *conn)
        .await?;

        let mut obl_count = 0usize;
        for (oid, tracks) in &state.obligations {
            let identity = tracks.identity.state_name();
            let screening = tracks.screening.state_name();
            let risk = tracks.risk.state_name();
            sqlx::query(
                r#"INSERT INTO "ob-poc".kyc_obligation_projection
                   (subject_root, obligation_id, basis_role, basis_jurisdiction, basis_cbu_role,
                    basis_source_event_id, identity_state, screening_state, risk_state, originating_event_id)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"#,
            )
            .bind(subject_root.0).bind(oid.0)
            .bind(&tracks.basis.role)
            .bind(&tracks.basis.jurisdiction)
            .bind(&tracks.basis.cbu_role)
            .bind(tracks.basis.source_event_id.0)
            .bind(identity).bind(screening).bind(risk)
            .bind(tracks.originating_event_id.0)
            .execute(&mut *conn).await?;
            obl_count += 1;
        }

        let mut subj_count = 0usize;
        for (sid, rollup) in &state.subjects {
            let (overall, decision_event_id) = match &rollup.overall_state {
                SubjectOverallState::Approved { by_event } => ("Approved", Some(by_event.0)),
                SubjectOverallState::Rejected { by_event } => ("Rejected", Some(by_event.0)),
                SubjectOverallState::AllTerminal => ("AllTerminal", None),
                SubjectOverallState::InProgress => ("InProgress", None),
            };
            let all_terminal = matches!(
                rollup.overall_state,
                SubjectOverallState::AllTerminal | SubjectOverallState::Approved { .. }
            );
            sqlx::query(
                r#"INSERT INTO "ob-poc".kyc_subject_rollup_projection
                   (subject_root, overall_state, obligation_count, all_terminal, decision_event_id)
                   VALUES ($1,$2,$3,$4,$5)"#,
            )
            .bind(sid.0)
            .bind(overall)
            .bind(rollup.obligations.len() as i32)
            .bind(all_terminal)
            .bind(decision_event_id)
            .execute(&mut *conn)
            .await?;
            subj_count += 1;
        }

        Ok(ObligationProjectionStats {
            obligations_written: obl_count,
            subjects_written: subj_count,
        })
    }
}

/// Self-contained outbox drainer for the obligation-graph projection.
pub struct PgKycObligationDrainer;

impl PgKycObligationDrainer {
    pub async fn drain_once(
        pool: &PgPool,
        registry: &FoldRegistry,
    ) -> Result<Option<SubjectId>, StoreError> {
        let mut tx = pool.begin().await?;
        let claimed = sqlx::query(
            r#"SELECT id, payload FROM "public".outbox
               WHERE effect_kind = $1 AND status = 'pending'
               ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1"#,
        )
        .bind(OBLIGATION_PROJECTION_EFFECT)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = claimed else {
            tx.rollback().await?;
            return Ok(None);
        };
        let id: Uuid = row.get("id");
        let payload: serde_json::Value = row.get("payload");
        let subject = payload
            .get("subject_root")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(SubjectId)
            .ok_or_else(|| {
                StoreError::Db(sqlx::Error::Decode(
                    "outbox payload missing/invalid subject_root".into(),
                ))
            })?;

        PgKycObligationProjector::rebuild_obligations(&mut tx, registry, subject).await?;
        sqlx::query(
            r#"UPDATE "public".outbox SET status = 'done', processed_at = now() WHERE id = $1"#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(subject))
    }

    pub async fn drain_all(
        pool: &PgPool,
        registry: &FoldRegistry,
        max: usize,
    ) -> Result<usize, StoreError> {
        let mut n = 0;
        while n < max {
            if Self::drain_once(pool, registry).await?.is_none() {
                break;
            }
            n += 1;
        }
        Ok(n)
    }
}

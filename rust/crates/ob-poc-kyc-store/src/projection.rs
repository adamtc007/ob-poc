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

use ob_poc_kyc_substrate::{
    fold_control_versioned, fold_obligations_versioned, FoldRegistry, IntentEvent, SubjectId,
    SubjectOverallState,
};

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

        Ok(ProjectionStats {
            edges_written: state.edges.len(),
        })
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
            // `Approved`/`Rejected` retired from `SubjectOverallState` TS.6
            // P2 (K-G7) — decisions live in `kyc_decision_records` now, not
            // this fold, so `overall_state` can only ever be one of these
            // two pre-decision values. `decision_event_id` is left
            // permanently NULL rather than dropping the column (TS.6 P1
            // ruling: non-destructive — a reader wanting the decision now
            // joins `kyc_decision_records` instead).
            //
            // TS.6 §5 (2026-08-22): DERIVED, not read off a stored
            // `rollup.overall_state` field. That field was only ever advanced
            // by the retired approve/reject fold arms, so after P2 it was
            // pinned at `InProgress` and this projection published
            // `all_terminal = false` for subjects whose obligations were all
            // terminal — while `decide.approve`'s own K-23 gate, which calls
            // `derive_subject_state`, correctly saw `AllTerminal`. One value,
            // two authorities, disagreeing. The field is gone; this is the
            // single authority.
            let derived = state.derive_subject_state(*sid);
            let (overall, decision_event_id): (&str, Option<uuid::Uuid>) = match &derived {
                SubjectOverallState::AllTerminal => ("AllTerminal", None),
                SubjectOverallState::InProgress => ("InProgress", None),
            };
            let all_terminal = matches!(derived, SubjectOverallState::AllTerminal);
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

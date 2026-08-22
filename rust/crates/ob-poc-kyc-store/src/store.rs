//! `PgKycEventStore` — the durable per-subject verb stream over Postgres.
//!
//! Implements the EOP-DD-KYCUBO-002 §3 append protocol. The substrate stays
//! pure: this crate hydrates rows into owned `IntentEvent`s and reuses the
//! source-agnostic folds (`fold_control_versioned`, …) verbatim.
//!
//! # The single-subject rule (§3.3)
//!
//! `append` touches exactly one `subject_root` stream — the one named by
//! `event.subject_root`. A transaction therefore holds at most one stream lock,
//! which is what makes the lock model deadlock-free.
//!
//! # seq allocation (§2 invariant)
//!
//! `seq` is read from `kyc_subject_streams.next_seq` under the row's `FOR UPDATE`
//! lock and bumped in the same transaction — never from a Postgres `SEQUENCE`.
//! Gap-free dense `seq` under rollback depends on this.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Row};

use ob_poc_kyc_substrate::{
    fold_control_versioned, fold_obligations_versioned, fold_type_registry, ControlState,
    FoldRegistry, IntentEvent, KycError, ObligationState, SubjectId, TypeRegistryState,
};

use crate::error::StoreError;
// TS.6 §1: the read side lives in the append-free `ob-poc-kyc-read` crate.
// This crate keeps `append` and delegates every read, so `PgKycEventStore`'s
// public surface is unchanged for its ~20 existing callers.
use ob_poc_kyc_read::PgKycEventReader;



/// Outcome of an append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppendOutcome {
    /// The `seq` of the event in its subject stream. For a deduped append this
    /// is the seq of the pre-existing event.
    pub seq: u64,
    /// The event's UUID (the ID that was or would have been inserted).
    pub event_id: ob_poc_kyc_substrate::EventId,
    /// True when the append was an idempotent no-op (the `idempotency_key`
    /// already existed for this subject) — the event was NOT re-inserted (F).
    pub deduped: bool,
}

/// The durable verb-stream store. Stateless — every method takes the connection
/// (inside the caller's transaction) explicitly.
pub struct PgKycEventStore;

impl PgKycEventStore {
    /// Load every event for `subject_root`, ordered by ascending `seq` (owned).
    ///
    /// Replaces the in-memory store's borrowed `events_for`. The caller folds
    /// over `&[&IntentEvent]` exactly as in the slice.
    /// Load every event for `subject_root`, ordered by ascending `seq` (owned).
    ///
    /// TS.6 §1: the implementation now lives in `ob-poc-kyc-read`, the
    /// append-free crate the Evaluation pack depends on. Delegated here so
    /// every pre-existing `PgKycEventStore::load_events` caller is unchanged.
    pub async fn load_events(
        conn: &mut PgConnection,
        subject_root: SubjectId,
    ) -> Result<Vec<IntentEvent>, StoreError> {
        PgKycEventReader::load_events(conn, subject_root).await
    }

    /// Transaction-time prefix load — see `ob_poc_kyc_read::PgKycEventReader`.
    pub async fn load_events_up_to_committed(
        conn: &mut PgConnection,
        subject_root: SubjectId,
        as_of_tx: DateTime<Utc>,
    ) -> Result<Vec<IntentEvent>, StoreError> {
        PgKycEventReader::load_events_up_to_committed(conn, subject_root, as_of_tx).await
    }

    /// Fold `ControlState` at a transaction-time bound — see
    /// `ob_poc_kyc_read::PgKycEventReader`.
    pub async fn recover_control_at(
        conn: &mut PgConnection,
        registry: &FoldRegistry,
        subject_root: SubjectId,
        as_of_tx: DateTime<Utc>,
    ) -> Result<ControlState, StoreError> {
        PgKycEventReader::recover_control_at(conn, registry, subject_root, as_of_tx).await
    }

    pub async fn append<V>(
        conn: &mut PgConnection,
        registry: &FoldRegistry,
        event: &IntentEvent,
        source_text: &str,
        validate: V,
    ) -> Result<AppendOutcome, StoreError>
    where
        V: FnOnce(&ControlState, &ObligationState, &TypeRegistryState) -> Result<(), KycError>,
    {
        let subject = event.subject_root;

        // 1. Ensure the stream row exists, then lock it FOR UPDATE.
        //    Concurrent INSERTs on the same PK serialize; the loser is a no-op.
        sqlx::query(
            r#"INSERT INTO "ob-poc".kyc_subject_streams (subject_root)
               VALUES ($1) ON CONFLICT (subject_root) DO NOTHING"#,
        )
        .bind(subject.0)
        .execute(&mut *conn)
        .await?;

        let stream_row = sqlx::query(
            r#"SELECT next_seq FROM "ob-poc".kyc_subject_streams
               WHERE subject_root = $1 FOR UPDATE"#,
        )
        .bind(subject.0)
        .fetch_one(&mut *conn)
        .await?;
        let next_seq: i64 = stream_row.get("next_seq");

        // 2. Idempotency (F): has this key already landed for this subject?
        let existing: Option<i64> = sqlx::query_scalar(
            r#"SELECT seq FROM "ob-poc".kyc_intent_events
               WHERE subject_root = $1 AND idempotency_key = $2"#,
        )
        .bind(subject.0)
        .bind(&event.idempotency_key.0)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(seq) = existing {
            return Ok(AppendOutcome {
                seq: seq as u64,
                event_id: event.id,
                deduped: true,
            });
        }

        // 3. Fold existing events (from seq 0 — checkpoint optimization deferred)
        //    and validate preconditions UNDER the lock (TOCTOU-safe, §3 step 3).
        let events = Self::load_events(conn, subject).await?;
        let refs: Vec<&IntentEvent> = events.iter().collect();
        let state = fold_control_versioned(&refs, registry)?; // KycError -> StoreError::Rejected
        let obligation_state = fold_obligations_versioned(&refs, registry)?;
        let type_registry = fold_type_registry(&refs);
        validate(&state, &obligation_state, &type_registry)?; // precondition failure -> StoreError::Rejected -> caller rolls back

        // 4. Insert at next_seq + bump (same txn: event and seq-bump commit together).
        insert_event(conn, event, next_seq, source_text).await?;
        sqlx::query(
            r#"UPDATE "ob-poc".kyc_subject_streams
               SET next_seq = next_seq + 1, updated_at = now()
               WHERE subject_root = $1"#,
        )
        .bind(subject.0)
        .execute(&mut *conn)
        .await?;

        // §3 step 5 (the outbox fan-out) REMOVED 2026-08-22. Events are
        // immutable, so replay from any point yields the same fold: a snapshot
        // rebuilt on demand from an immutable stream is always correct, and a
        // queue telling you WHEN to rebuild adds nothing but a place to jam.
        // It jammed — the drainers hard-error on any event whose lexicon hash
        // is not in the FoldRegistry, and a failed drain leaves the row
        // `pending`, so the next claim takes the same row: any manifest bump
        // stranded the whole pre-bump backlog behind a head-of-line poison
        // pill. Folding on demand uses current rules over immutable events and
        // never asks what version a queued row was written under.
        // `PgKycProjector` (K-34, fold-whole-stream + full-replace) is
        // unchanged and is now called directly. Its obligation counterpart
        // was removed D2.0 §5, 2026-08-22 (see `projection.rs`).

        Ok(AppendOutcome {
            seq: next_seq as u64,
            event_id: event.id,
            deduped: false,
        })
    }

}

// ── Row ↔ IntentEvent mapping ─────────────────────────────────────────────────

async fn insert_event(
    conn: &mut PgConnection,
    event: &IntentEvent,
    seq: i64,
    source_text: &str,
) -> Result<(), StoreError> {
    let actor = serde_json::to_value(&event.actor).map_err(sqlx_json)?;
    let target = serde_json::to_value(&event.target).map_err(sqlx_json)?;
    let captured = serde_json::to_value(&event.captured_effects).map_err(sqlx_json)?;

    sqlx::query(
        r#"INSERT INTO "ob-poc".kyc_intent_events
            (subject_root, seq, event_id, verb_fqn, lexicon_hash, actor, authority,
             target, payload, payload_hash, idempotency_key, causation_id,
             correlation_id, as_of, captured_effects, source_text)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)"#,
    )
    .bind(event.subject_root.0)
    .bind(seq)
    .bind(event.id.0)
    .bind(event.verb_fqn.as_str())
    .bind(event.lexicon_hash.to_hex())
    .bind(actor)
    .bind(&event.authority.0)
    .bind(target)
    .bind(&event.payload)
    .bind(event.payload_hash.to_hex())
    .bind(&event.idempotency_key.0)
    .bind(event.causation_id.map(|e| e.0))
    .bind(event.correlation_id)
    .bind(event.as_of)
    .bind(captured)
    .bind(source_text)
    .execute(&mut *conn)
    .await?;
    Ok(())
}



fn sqlx_json(e: serde_json::Error) -> sqlx::Error {
    sqlx::Error::Encode(Box::new(e))
}

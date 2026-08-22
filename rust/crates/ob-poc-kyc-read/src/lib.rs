//! Read-only access to the dsl.kyc fact stream (EOP-DD-KYCUBO-TS.6 §1).
//!
//! **This crate contains no append path, and must never gain one.** That is
//! its entire reason to exist. The TS.6 Evaluation pack (`ob-poc-kyc-decide`)
//! depends on this crate instead of `ob-poc-kyc-store`, so
//! "the evaluation pack cannot write a fact" is a property of the dependency
//! graph — checkable by `cargo tree` — rather than a property of what each
//! op's body happens to call.
//!
//! The 2026-08-22 reconciliation proved the previous arrangement did not hold:
//! `ob-poc-kyc-decide` depended directly on `ob-poc-kyc-store`, whose
//! `PgKycEventStore::append` is `pub`, so a probe inside the evaluation pack
//! appended a real event with `ob-poc-kyc-seam` nowhere in its tree. The
//! dep-gate forbade an edge (`ob-poc-kyc-seam`) that is not the write path.
//!
//! `ob-poc-kyc-store` depends on this crate and re-exports these functions via
//! `PgKycEventStore`, so every pre-existing caller is unaffected.
//!
//! # Known open ruling — TS.6 §2
//!
//! §2 says *"Evaluation never reads: raw assertions."* `load_events` returns
//! raw `IntentEvent`s, and the evaluation pack currently folds them itself to
//! derive `SubjectOverallState`. That is a **§2 read-boundary divergence,
//! reported 2026-08-22 and awaiting ruling** — it is NOT sanctioned by this
//! crate existing. The §2-conformant alternative (evaluation reads the
//! constructed `kyc_*_projection` tables) was not taken because those
//! projections are drained asynchronously, so gating K-23 on them could
//! approve against stale state. This crate closes the WRITE divergence only.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    fold_control_versioned, AuthorityRef, CapturedEffect, ControlState, EventId, FoldRegistry,
    Hash, IdemKey, IntentEvent, Principal, SubjectId, TargetBinding, VerbFqn,
};

mod error;
pub use error::StoreError;

/// The `kyc_intent_events` column list, in the order `row_to_event` reads them.
/// Single source of truth so every SELECT and the row mapper cannot drift.
const EVENT_COLUMNS: &str = "subject_root, seq, event_id, verb_fqn, lexicon_hash, actor, \
    authority, target, payload, payload_hash, idempotency_key, causation_id, correlation_id, \
    as_of, captured_effects, committed_at";

/// Read-only reader over the durable verb stream. Stateless — every method
/// takes the connection (inside the caller's transaction) explicitly.
///
/// Deliberately has NO `append`: see the module docs.
pub struct PgKycEventReader;

impl PgKycEventReader {
    /// Load every event for `subject_root`, ordered by ascending `seq` (owned).
    ///
    /// The caller folds over `&[&IntentEvent]` exactly as in the slice.
    pub async fn load_events(
        conn: &mut PgConnection,
        subject_root: SubjectId,
    ) -> Result<Vec<IntentEvent>, StoreError> {
        let rows = sqlx::query(&format!(
            r#"SELECT {EVENT_COLUMNS} FROM "ob-poc".kyc_intent_events
               WHERE subject_root = $1 ORDER BY seq ASC"#
        ))
        .bind(subject_root.0)
        .fetch_all(&mut *conn)
        .await?;

        rows.iter().map(row_to_event).collect()
    }

    /// Load the **transaction-time prefix** for `subject_root`: the events that
    /// had been committed at or before `as_of_tx`, as a TRUE seq-prefix
    /// (B1/D1/K-33).
    ///
    /// This is the recovery axis. The query is deliberately a seq-prefix —
    /// `seq <` the first event whose `committed_at` exceeds the bound — not a
    /// raw `committed_at <= T` filter. So even if `committed_at` were ever
    /// non-monotonic with `seq` (a wall-clock regression), the result is always
    /// a contiguous prefix and the fold is never holey. `as_of` (valid-time) is
    /// **not** the filter — only `committed_at` (transaction-time) is (B1).
    pub async fn load_events_up_to_committed(
        conn: &mut PgConnection,
        subject_root: SubjectId,
        as_of_tx: DateTime<Utc>,
    ) -> Result<Vec<IntentEvent>, StoreError> {
        let rows = sqlx::query(&format!(
            r#"SELECT {EVENT_COLUMNS} FROM "ob-poc".kyc_intent_events
               WHERE subject_root = $1
                 AND seq < COALESCE(
                     (SELECT MIN(seq) FROM "ob-poc".kyc_intent_events
                      WHERE subject_root = $1 AND committed_at > $2),
                     9223372036854775807)
               ORDER BY seq ASC"#
        ))
        .bind(subject_root.0)
        .bind(as_of_tx)
        .fetch_all(&mut *conn)
        .await?;

        rows.iter().map(row_to_event).collect()
    }

    /// Fold `ControlState` from the transaction-time prefix at `as_of_tx`.
    pub async fn recover_control_at(
        conn: &mut PgConnection,
        registry: &FoldRegistry,
        subject_root: SubjectId,
        as_of_tx: DateTime<Utc>,
    ) -> Result<ControlState, StoreError> {
        let events = Self::load_events_up_to_committed(conn, subject_root, as_of_tx).await?;
        let refs: Vec<&IntentEvent> = events.iter().collect();
        fold_control_versioned(&refs, registry).map_err(StoreError::from)
    }
}

/// Load the ordered `source_text` history for `subject_root` (T1 — KIT-1
/// replayability). `NULL` entries are pre-T1 rows pending backfill.
pub async fn load_source_text_history(
    conn: &mut PgConnection,
    subject_root: SubjectId,
) -> Result<Vec<(u64, Option<String>)>, StoreError> {
    let rows = sqlx::query(
        r#"SELECT seq, source_text FROM "ob-poc".kyc_intent_events
           WHERE subject_root = $1 ORDER BY seq ASC"#,
    )
    .bind(subject_root.0)
    .fetch_all(&mut *conn)
    .await?;

    Ok(rows
        .iter()
        .map(|row| {
            let seq: i64 = row.get("seq");
            let source_text: Option<String> = row.get("source_text");
            (seq as u64, source_text)
        })
        .collect())
}

/// Rehydrate one `kyc_intent_events` row into an owned `IntentEvent`.
/// `pub` so `ob-poc-kyc-store`'s append path can reuse the identical mapping —
/// one row mapper, never two to drift apart.
pub fn row_to_event(row: &sqlx::postgres::PgRow) -> Result<IntentEvent, StoreError> {
    let subject: Uuid = row.get("subject_root");
    let seq: i64 = row.get("seq");
    let rehydrate = |reason: String| StoreError::Rehydrate {
        subject,
        seq,
        reason,
    };

    let lexicon_hash = Hash::from_hex(&row.get::<String, _>("lexicon_hash"))
        .map_err(|e| rehydrate(format!("lexicon_hash: {e}")))?;
    let payload_hash = Hash::from_hex(&row.get::<String, _>("payload_hash"))
        .map_err(|e| rehydrate(format!("payload_hash: {e}")))?;
    let actor: Principal =
        serde_json::from_value(row.get("actor")).map_err(|e| rehydrate(format!("actor: {e}")))?;
    let target: TargetBinding =
        serde_json::from_value(row.get("target")).map_err(|e| rehydrate(format!("target: {e}")))?;
    let captured_effects: Vec<CapturedEffect> = serde_json::from_value(row.get("captured_effects"))
        .map_err(|e| rehydrate(format!("captured_effects: {e}")))?;

    Ok(IntentEvent {
        id: EventId(row.get("event_id")),
        seq: seq as u64,
        subject_root: SubjectId(subject),
        verb_fqn: VerbFqn(row.get("verb_fqn")),
        lexicon_hash,
        actor,
        authority: AuthorityRef(row.get("authority")),
        target,
        payload: row.get("payload"),
        payload_hash,
        idempotency_key: IdemKey(row.get("idempotency_key")),
        causation_id: row.get::<Option<Uuid>, _>("causation_id").map(EventId),
        correlation_id: row.get("correlation_id"),
        as_of: row.get("as_of"),
        committed_at: row.get("committed_at"),
        captured_effects,
    })
}

/// The column list, exposed so `ob-poc-kyc-store`'s append-side SELECT
/// (dedupe lookup) uses the identical projection — one column list, never two
/// to drift apart.
pub const fn event_columns() -> &'static str {
    EVENT_COLUMNS
}

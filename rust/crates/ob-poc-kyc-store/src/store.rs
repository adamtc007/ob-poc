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
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    fold_control_versioned, AuthorityRef, CapturedEffect, ControlState, EventId, FoldRegistry,
    Hash, IdemKey, IntentEvent, KycError, Principal, SubjectId, TargetBinding, VerbFqn,
};

use crate::error::StoreError;

/// The `kyc_intent_events` column list, in the order `row_to_event` reads them.
/// Single source of truth so every SELECT and the row mapper cannot drift.
const EVENT_COLUMNS: &str = "subject_root, seq, event_id, verb_fqn, lexicon_hash, actor, \
    authority, target, payload, payload_hash, idempotency_key, causation_id, correlation_id, \
    as_of, captured_effects";

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
    /// had been committed at or before `as_of_tx`, as a TRUE seq-prefix (B1/D1/K-33).
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
                     (SELECT min(seq) FROM "ob-poc".kyc_intent_events
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

    /// Recover the control fold **as it stood at transaction-time `as_of_tx`**
    /// (K-33). Loads the committed-prefix and folds it via the version registry.
    /// `recover_control_at(subject, now)` equals folding the whole stream.
    pub async fn recover_control_at(
        conn: &mut PgConnection,
        registry: &FoldRegistry,
        subject_root: SubjectId,
        as_of_tx: DateTime<Utc>,
    ) -> Result<ControlState, StoreError> {
        let events = Self::load_events_up_to_committed(conn, subject_root, as_of_tx).await?;
        let refs: Vec<&IntentEvent> = events.iter().collect();
        Ok(fold_control_versioned(&refs, registry)?)
    }

    /// The §3 append protocol. Runs inside the caller's transaction (`conn` is
    /// inside a `BEGIN`). Future seam chokepoint `append_in_scope` (§3.6).
    ///
    /// 1. Lock the subject's stream row `FOR UPDATE` (the ordering domain, Q6).
    /// 2. Idempotency: if the key already landed for this subject, return its seq.
    /// 3. Fold existing events via the registry; run `validate` against the
    ///    folded state — the precondition check happens UNDER the lock (TOCTOU-safe).
    /// 4. Insert the event at `next_seq`; bump `next_seq` (same txn = atomic).
    ///
    /// `validate` is the precondition policy (e.g. proof-ratchet, reconcile). The
    /// store stays decoupled from the lexicon; the caller supplies the check.
    pub async fn append<V>(
        conn: &mut PgConnection,
        registry: &FoldRegistry,
        event: &IntentEvent,
        validate: V,
    ) -> Result<AppendOutcome, StoreError>
    where
        V: FnOnce(&ControlState) -> Result<(), KycError>,
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
        validate(&state)?; // precondition failure -> StoreError::Rejected -> caller rolls back

        // 4. Insert at next_seq + bump (same txn: event and seq-bump commit together).
        insert_event(conn, event, next_seq).await?;
        sqlx::query(
            r#"UPDATE "ob-poc".kyc_subject_streams
               SET next_seq = next_seq + 1, updated_at = now()
               WHERE subject_root = $1"#,
        )
        .bind(subject.0)
        .execute(&mut *conn)
        .await?;

        // 5. Enqueue the projection effect(s) on the outbox, in the same txn (§3 step 5).
        //    The drainer re-derives the subject's projections by folding the stream.
        Self::enqueue_projection_effects(conn, event, next_seq).await?;

        Ok(AppendOutcome {
            seq: next_seq as u64,
            event_id: event.id,
            deduped: false,
        })
    }

    /// Enqueue one outbox row per projection effect-kind for a freshly appended
    /// event (the §3 step-5 fan-out). Keyed `subject:seq` so a retried append is
    /// idempotent at the outbox (`ON CONFLICT DO NOTHING`); the projector is a
    /// full-rebuild, so one "subject changed" notification per event suffices.
    /// Runs in the append transaction — the effect commits atomically with the event.
    async fn enqueue_projection_effects(
        conn: &mut PgConnection,
        event: &IntentEvent,
        seq: i64,
    ) -> Result<(), StoreError> {
        let payload = serde_json::json!({ "subject_root": event.subject_root.0 });
        let idem = format!("{}:{}", event.subject_root.0, seq);
        for effect_kind in crate::projection::PROJECTION_EFFECT_KINDS {
            sqlx::query(
                r#"INSERT INTO "public".outbox
                    (id, trace_id, envelope_version, effect_kind, payload, idempotency_key)
                   VALUES ($1, $2, 1, $3, $4, $5)
                   ON CONFLICT (idempotency_key, effect_kind) DO NOTHING"#,
            )
            .bind(Uuid::new_v4())
            .bind(event.correlation_id)
            .bind(*effect_kind)
            .bind(&payload)
            .bind(&idem)
            .execute(&mut *conn)
            .await?;
        }
        Ok(())
    }
}

// ── Row ↔ IntentEvent mapping ─────────────────────────────────────────────────

async fn insert_event(
    conn: &mut PgConnection,
    event: &IntentEvent,
    seq: i64,
) -> Result<(), StoreError> {
    let actor = serde_json::to_value(&event.actor).map_err(sqlx_json)?;
    let target = serde_json::to_value(&event.target).map_err(sqlx_json)?;
    let captured = serde_json::to_value(&event.captured_effects).map_err(sqlx_json)?;

    sqlx::query(
        r#"INSERT INTO "ob-poc".kyc_intent_events
            (subject_root, seq, event_id, verb_fqn, lexicon_hash, actor, authority,
             target, payload, payload_hash, idempotency_key, causation_id,
             correlation_id, as_of, captured_effects)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)"#,
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
    .execute(&mut *conn)
    .await?;
    Ok(())
}

fn row_to_event(row: &sqlx::postgres::PgRow) -> Result<IntentEvent, StoreError> {
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
        captured_effects,
    })
}

fn sqlx_json(e: serde_json::Error) -> sqlx::Error {
    sqlx::Error::Encode(Box::new(e))
}

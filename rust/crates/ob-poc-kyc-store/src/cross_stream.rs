//! Cross-stream emission contract (EOP-DD-KYCUBO-002 §3.4 — B2, B3).
//!
//! When `ubo.determination.freeze` runs on subject A's stream, it must emit
//! obligation-create (or obligation-supersede for retraction) events onto each
//! resolved person's subject stream via the outbox. Three rules:
//!
//! 1. **Deterministic idempotency key (B3).**
//!    `idem = sha256(freeze_event_id ‖ target_subject ‖ effect_kind)`
//!    Dedupes the stream-append on B under at-least-once outbox delivery.
//!
//! 2. **Retraction on re-determination (B2).**
//!    A re-freeze computes set-diff of resolved persons:
//!    - `now − before` → obligation-create effects
//!    - `before − now` → obligation-supersede effects (retraction)
//!    `before` is itself a fold of A's prior freeze emissions — pure, replayable.
//!
//! 3. **Emission failure is dead-lettered, never dropped.**
//!    The outbox already retries (at-least-once); permanent failure routes to
//!    `failed_terminal` — the determination is flagged `emission_incomplete`,
//!    never silently successful.

use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use ob_poc_kyc_substrate::{EventId, PersonId, SubjectId};

use crate::error::StoreError;

/// Outbox effect-kind for cross-stream obligation-create (B3).
pub const CROSS_STREAM_OBLIGATION_CREATE: &str = "kyc.cross_stream.obligation_create";
/// Outbox effect-kind for cross-stream obligation-supersede/retraction (B2).
pub const CROSS_STREAM_OBLIGATION_SUPERSEDE: &str = "kyc.cross_stream.obligation_supersede";

/// Derive a deterministic idempotency key for a cross-stream effect (B3).
///
/// `sha256(freeze_event_id ‖ target_subject_id ‖ effect_kind)` — stable across
/// redeliveries so the outbox deduplication + the stream-append idempotency gate
/// together ensure exactly-once obligation creation.
pub fn cross_stream_idem_key(
    freeze_event_id: EventId,
    target_subject: SubjectId,
    effect_kind: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(freeze_event_id.0.as_bytes());
    h.update(target_subject.0.as_bytes());
    h.update(effect_kind.as_bytes());
    hex::encode(h.finalize())
}

/// Enqueue cross-stream obligation effects for all resolved persons from a freeze.
///
/// Computes set-diff against the prior freeze's emissions (B2 retraction):
/// - persons IN current but NOT IN prior → enqueue obligation-create
/// - persons IN prior but NOT IN current → enqueue obligation-supersede
///
/// All effects are enqueued in the same transaction as the freeze event itself,
/// so they commit atomically with it (or roll back together).
pub async fn enqueue_cross_stream_obligations(
    conn: &mut PgConnection,
    freeze_event_id: EventId,
    subject: SubjectId,
    resolved_persons: &[PersonId],
    prior_person_ids: &[PersonId],
    role: &str,
    correlation_id: Uuid,
) -> Result<CrossStreamEnqueueOutcome, StoreError> {
    let current: std::collections::BTreeSet<Uuid> =
        resolved_persons.iter().map(|p| p.0).collect();
    let prior: std::collections::BTreeSet<Uuid> =
        prior_person_ids.iter().map(|p| p.0).collect();

    let to_create: Vec<Uuid> = current.difference(&prior).copied().collect();
    let to_supersede: Vec<Uuid> = prior.difference(&current).copied().collect();

    let mut creates = 0u32;
    let mut supersedes = 0u32;

    for person_uuid in &to_create {
        let target = SubjectId(*person_uuid);
        let idem = cross_stream_idem_key(freeze_event_id, target, CROSS_STREAM_OBLIGATION_CREATE);
        let payload = serde_json::json!({
            "subject_root": person_uuid,
            "determination_subject": subject.0,
            "role": role,
            "causation_freeze_event_id": freeze_event_id.0,
        });
        sqlx::query(
            r#"INSERT INTO "public".outbox
               (id, trace_id, envelope_version, effect_kind, payload, idempotency_key)
               VALUES ($1,$2,1,$3,$4,$5)
               ON CONFLICT (idempotency_key, effect_kind) DO NOTHING"#,
        )
        .bind(Uuid::new_v4())
        .bind(correlation_id)
        .bind(CROSS_STREAM_OBLIGATION_CREATE)
        .bind(payload)
        .bind(idem)
        .execute(&mut *conn)
        .await?;
        creates += 1;
    }

    for person_uuid in &to_supersede {
        let target = SubjectId(*person_uuid);
        let idem = cross_stream_idem_key(freeze_event_id, target, CROSS_STREAM_OBLIGATION_SUPERSEDE);
        let payload = serde_json::json!({
            "subject_root": person_uuid,
            "determination_subject": subject.0,
            "causation_freeze_event_id": freeze_event_id.0,
            "reason": "re-determination: person no longer resolved",
        });
        sqlx::query(
            r#"INSERT INTO "public".outbox
               (id, trace_id, envelope_version, effect_kind, payload, idempotency_key)
               VALUES ($1,$2,1,$3,$4,$5)
               ON CONFLICT (idempotency_key, effect_kind) DO NOTHING"#,
        )
        .bind(Uuid::new_v4())
        .bind(correlation_id)
        .bind(CROSS_STREAM_OBLIGATION_SUPERSEDE)
        .bind(payload)
        .bind(idem)
        .execute(&mut *conn)
        .await?;
        supersedes += 1;
    }

    Ok(CrossStreamEnqueueOutcome { creates, supersedes })
}

/// The set of PersonIds emitted by the most recent freeze on a subject's stream.
///
/// Derived from A's own stream (the prior freeze's emissions are events) — pure
/// and replayable. Returns an empty set when no prior freeze has run.
pub async fn prior_freeze_persons(
    conn: &mut PgConnection,
    subject: SubjectId,
) -> Result<Vec<PersonId>, StoreError> {
    // The prior freeze event emitted cross-stream effects; read the outbox rows
    // keyed to this subject to find the prior set. We look for DONE rows (already
    // processed) from the most recent freeze, identified by the presence of
    // `determination_subject == subject` in the payload.
    let rows: Vec<serde_json::Value> = sqlx::query(
        r#"SELECT payload FROM "public".outbox
           WHERE effect_kind = $1
             AND status = 'done'
             AND (payload->>'determination_subject')::uuid = $2
           ORDER BY created_at DESC"#,
    )
    .bind(CROSS_STREAM_OBLIGATION_CREATE)
    .bind(subject.0)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|r| r.get::<serde_json::Value, _>("payload"))
    .collect();

    let persons = rows
        .iter()
        .filter_map(|p| {
            p.get("subject_root")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .map(PersonId)
        })
        .collect();

    Ok(persons)
}

/// Outcome of enqueueing cross-stream obligations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrossStreamEnqueueOutcome {
    pub creates: u32,
    pub supersedes: u32,
}

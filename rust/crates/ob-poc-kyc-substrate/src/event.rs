//! Verb-event contract (§2 of EOP-DD-KYCUBO-001).
//!
//! `IntentEvent` is the append-only, intent-native record. In the slice it
//! lives in `InMemoryEventStore`; in W1-proper the identical shape becomes
//! the durable `kyc_intent_events` table — callers are unchanged.
//!
//! Three properties enforced by construction (§2):
//! 1. No state without semantic cause (K-35): every fold transition is keyed
//!    off an event carrying `actor` + `authority` + `target`.
//! 2. Deterministic replay (K-16/18/33): `as_of` is frozen into the event;
//!    `captured_effects` holds any external-lookup result.  Re-running the
//!    fold yields bit-identical state.
//! 3. Effects dispatch once (H5): replay folds state only; it never
//!    re-dispatches effects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::KycError;
use crate::types::{
    AuthorityRef, EventId, Hash, IdemKey, Principal, SubjectId, TargetBinding, VerbFqn,
};

// ── Captured external effect ──────────────────────────────────────────────────

/// Result of an external lookup captured *at first execution* so that replay
/// never re-calls the external service (Q6).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedEffect {
    /// Logical kind, e.g. "screening_result" or "registry_lookup".
    pub kind: String,
    /// Lookup key (entity LEI, entity id, …).
    pub key: String,
    /// Captured result value.
    pub value: serde_json::Value,
}

// ── Intent event ──────────────────────────────────────────────────────────────

/// Append-only, intent-native record.  The ordering domain is per-`subject_root`
/// (Q6, not global).  `state = fold(events_for(subject_root))`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentEvent {
    /// Unique event id (UUID v4; becomes PK in the durable table).
    pub id: EventId,
    /// Dense, per-subject monotonic sequence.
    pub seq: u64,
    /// The determination root this stream belongs to (ordering domain, Q6).
    pub subject_root: SubjectId,

    /// Verb FQN, e.g. `"ubo.edge.verify"`.
    pub verb_fqn: VerbFqn,
    /// Content hash of the lexicon entry this event was written against (Q7).
    pub lexicon_hash: Hash,

    /// Actor that invoked the verb.
    pub actor: Principal,
    /// Object-capability authorising the move (K-17, K-35).
    pub authority: AuthorityRef,
    /// The edge / node / obligation acted on.
    pub target: TargetBinding,

    /// Verb arguments serialised as JSON.
    pub payload: serde_json::Value,
    /// SHA-256 of `payload` (content integrity; needed for K-18 graph hash).
    pub payload_hash: Hash,

    /// Deduplication key. Two appends with the same key are idempotent.
    pub idempotency_key: IdemKey,
    /// The event that caused this one (for causation chains).
    pub causation_id: Option<EventId>,
    /// Distributed trace correlation id.
    pub correlation_id: Uuid,

    /// **Frozen** point-in-time for this event (Q6 — never `now()` inside a verb).
    pub as_of: DateTime<Utc>,
    /// External-lookup results captured at first execution; replayed verbatim (Q6).
    pub captured_effects: Vec<CapturedEffect>,
}

impl IntentEvent {
    /// Create a minimal intent event (7 required args; use builder setters for the rest).
    pub fn new(
        subject_root: SubjectId,
        verb_fqn: impl Into<VerbFqn>,
        actor: Principal,
        authority: AuthorityRef,
        target: TargetBinding,
        payload: serde_json::Value,
        as_of: DateTime<Utc>,
    ) -> Self {
        let payload_hash = Hash::of_json(&payload);
        Self {
            id: EventId::new(),
            seq: 0,
            subject_root,
            verb_fqn: verb_fqn.into(),
            lexicon_hash: Hash::of(b"default"),
            actor,
            authority,
            target,
            payload_hash,
            payload,
            idempotency_key: IdemKey::from_uuid(Uuid::new_v4()),
            causation_id: None,
            correlation_id: Uuid::new_v4(),
            as_of,
            captured_effects: vec![],
        }
    }

    /// Set the sequence number (normally assigned by the store; override for testing).
    pub fn with_seq(mut self, seq: u64) -> Self {
        self.seq = seq;
        self
    }

    /// Set the lexicon hash (content address of the verb definition used, Q7).
    pub fn with_lexicon_hash(mut self, h: Hash) -> Self {
        self.lexicon_hash = h;
        self
    }

    /// Set a deterministic idempotency key.
    pub fn with_idempotency_key(mut self, k: IdemKey) -> Self {
        self.idempotency_key = k;
        self
    }

    pub fn with_causation(mut self, id: EventId) -> Self {
        self.causation_id = Some(id);
        self
    }
}

// ── Store trait ───────────────────────────────────────────────────────────────

/// The only interface the fold functions depend on.  In-memory for the slice;
/// durable postgres in W1-proper — callers are unchanged (§6 seam).
pub trait KycEventStore: Send + Sync {
    /// Append `event` to the subject's stream.  Returns the assigned `seq`.
    ///
    /// # Idempotency
    /// If an event with the same `idempotency_key` already exists for
    /// `event.subject_root`, returns `Ok(existing_seq)` without re-appending.
    fn append(&mut self, event: IntentEvent) -> Result<u64, KycError>;

    /// Return all events for `subject_root` ordered by ascending `seq`.
    fn events_for(&self, subject_root: SubjectId) -> Vec<&IntentEvent>;

    /// Return events for `subject_root` with `seq <= up_to_seq` (point-in-time
    /// replay, Q6 / K-33).
    fn events_for_up_to(&self, subject_root: SubjectId, up_to_seq: u64) -> Vec<&IntentEvent>;
}

// ── In-memory implementation ──────────────────────────────────────────────────

/// Slice-tier implementation: `Vec<IntentEvent>` per subject root.
/// Replaced by the durable Postgres store in W1-proper with **no caller change**
/// (both implement `KycEventStore`).
#[derive(Debug, Default)]
pub struct InMemoryEventStore {
    /// Events grouped by subject root, in append order.
    streams: std::collections::BTreeMap<SubjectId, Vec<IntentEvent>>,
    /// Idempotency index: (subject_root, idempotency_key) → seq.
    /// BTreeMap for deterministic lookup order (no correctness impact here,
    /// but consistent with the fold-path determinism policy).
    idem: std::collections::BTreeMap<(SubjectId, String), u64>,
}

impl KycEventStore for InMemoryEventStore {
    fn append(&mut self, mut event: IntentEvent) -> Result<u64, KycError> {
        let idem_key = (event.subject_root, event.idempotency_key.0.clone());
        if let Some(&existing_seq) = self.idem.get(&idem_key) {
            return Ok(existing_seq);
        }

        let stream = self.streams.entry(event.subject_root).or_default();
        let seq = stream.len() as u64;
        event.seq = seq;

        self.idem.insert(idem_key, seq);
        stream.push(event);
        Ok(seq)
    }

    fn events_for(&self, subject_root: SubjectId) -> Vec<&IntentEvent> {
        self.streams
            .get(&subject_root)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    fn events_for_up_to(&self, subject_root: SubjectId, up_to_seq: u64) -> Vec<&IntentEvent> {
        self.streams
            .get(&subject_root)
            .map(|v| v.iter().filter(|e| e.seq <= up_to_seq).collect())
            .unwrap_or_default()
    }
}

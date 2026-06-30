//! The runtime↔store seam: chokepoint + identity mapping.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc_kyc_store::{AppendOutcome, PgKycEventStore, StoreError};
use ob_poc_kyc_substrate::{
    AuthorityRef, ControlState, FoldRegistry, Hash, IdemKey, IntentEvent, KycError, Principal,
    SubjectId, TargetBinding, VerbFqn,
};
use sem_os_core::principal::Principal as RuntimePrincipal;

/// Fixed namespace for deriving a *stable* substrate actor UUID from a non-UUID
/// runtime `actor_id`. Deterministic: the same string always maps to the same
/// UUID, so replay/audit are reproducible.
const SEAM_ACTOR_NS: Uuid = Uuid::from_bytes([
    0x6b, 0x79, 0x63, 0x2d, 0x73, 0x65, 0x61, 0x6d, 0x2d, 0x61, 0x63, 0x74, 0x6f, 0x72, 0x00, 0x01,
]);

/// Map the runtime principal (string `actor_id`, role vec, claims, tenancy) to
/// the substrate's thin principal (UUID `actor_id`, single primary role).
///
/// The lossy fields are deliberate: the substrate principal is identity only.
/// The rich authorising context (the object-capability, full role set) rides
/// separately in [`IntentEvent::authority`] (K-17, K-35), which the caller
/// supplies — so nothing audit-relevant is dropped here.
///
/// A non-UUID `actor_id` is mapped to a deterministic v5 UUID rather than
/// failing — the runtime identity space (JWT subjects, service ids) is wider
/// than UUIDs.
pub fn map_principal(p: &RuntimePrincipal) -> Principal {
    let actor_id = Uuid::parse_str(&p.actor_id)
        .unwrap_or_else(|_| Uuid::new_v5(&SEAM_ACTOR_NS, p.actor_id.as_bytes()));
    Principal {
        actor_id,
        role: p.roles.first().cloned().unwrap_or_default(),
    }
}

/// A verb-specific draft of an intent event. The caller fills the domain bits;
/// [`Self::into_event`] stamps the execution identity (actor / correlation /
/// idempotency).
///
/// Taking the identity triple explicitly (rather than the whole
/// `VerbExecutionContext`) keeps the seam decoupled from the runtime context
/// struct and testable without constructing one.
pub struct IntentEventDraft {
    pub verb_fqn: VerbFqn,
    pub subject_root: SubjectId,
    pub target: TargetBinding,
    pub payload: serde_json::Value,
    /// Object-capability authorising the move (K-17, K-35).
    pub authority: AuthorityRef,
    /// Content hash of the lexicon entry this verb was written against (Q7).
    pub lexicon_hash: Hash,
    /// Frozen at verb **entry** by the caller — never `now()` inside the op (§4 step 1).
    pub as_of: DateTime<Utc>,
}

impl IntentEventDraft {
    /// Stamp execution identity onto the draft, producing the appendable event.
    ///
    /// `execution_id` becomes the idempotency key, so a retried execution (same
    /// id) dedupes at the store rather than double-appending (F).
    pub fn into_event(
        self,
        principal: &RuntimePrincipal,
        correlation_id: Uuid,
        execution_id: Uuid,
    ) -> IntentEvent {
        let mut event = IntentEvent::new(
            self.subject_root,
            self.verb_fqn,
            map_principal(principal),
            self.authority,
            self.target,
            self.payload,
            self.as_of,
        )
        .with_lexicon_hash(self.lexicon_hash)
        .with_idempotency_key(IdemKey::new(execution_id.to_string()));
        event.correlation_id = correlation_id;
        event
    }
}

/// The §3.6 chokepoint: run the §3 append inside the Sequencer-owned
/// `TransactionScope`. The event commits/rolls back atomically with whatever
/// else the verb did in the same scope (the shadow-write, in §6 step 1).
///
/// A thin bridge `scope.executor()` → the store. Holds **no** logic — the
/// store orchestrates lock/fold/insert; the substrate folds; `validate` is the
/// precondition policy the caller supplies (typically
/// `substrate::check_control_preconditions(entry, state, event)`).
pub async fn append_in_scope<V>(
    scope: &mut dyn TransactionScope,
    registry: &FoldRegistry,
    event: &IntentEvent,
    validate: V,
) -> Result<AppendOutcome, StoreError>
where
    V: FnOnce(&ControlState) -> Result<(), KycError>,
{
    PgKycEventStore::append(scope.executor(), registry, event, validate).await
}

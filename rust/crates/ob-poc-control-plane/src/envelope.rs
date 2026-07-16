//! G10 — Runtime Execution Envelope (V&S §6.10) and proof-carrying
//! construction (V&S §9.4).
//!
//! The tollgate (§7) is enforced by the type system here, not by a runtime
//! checklist: `ExecutionEnvelope::seal` is the *only* constructor, is
//! `pub(crate)` (unreachable outside this crate — see the trybuild fixture
//! in `tests/trybuild/`), and requires every gate's success-form proof by
//! signature. There is no code path from a rejection to an envelope: each
//! parameter's type is a proof that can only be produced by its own gate
//! module succeeding (§9.4 design consequence).
//!
//! T1 defines the shape only. T4.1 wires envelope admission at the
//! `VerbExecutionPort`; T4.2 adds persistence/single-use/TTL; T4.3 adds
//! pre-state pinning enforcement.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::authority_gate::Authorised;
use crate::dag_proof::LegalTransition;
use crate::entity_binding::BoundEntities;
use crate::evidence_gate::EvidenceSufficient;
use crate::intent_admission::AdmittedIntent;
use crate::pack_resolution::ResolvedPack;
use crate::proof::CompiledRunbookRef;
use crate::snapshot::SnapshotPins;
use crate::write_set::WriteSetProof;

/// The window during which a sealed envelope may be consumed exactly once
/// (V&S §6.10.2). T1 defines the shape only; T4.2 wires the persistence
/// layer (`control_plane_envelopes`) that enforces single-use + TTL against
/// this window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ValidityWindow {
    not_before: DateTime<Utc>,
    not_after: DateTime<Utc>,
}

impl ValidityWindow {
    pub fn new(not_before: DateTime<Utc>, not_after: DateTime<Utc>) -> Self {
        assert!(
            not_before <= not_after,
            "ValidityWindow: not_before must precede not_after"
        );
        Self {
            not_before,
            not_after,
        }
    }

    pub fn not_before(&self) -> DateTime<Utc> {
        self.not_before
    }

    pub fn not_after(&self) -> DateTime<Utc> {
        self.not_after
    }

    pub fn contains(&self, instant: DateTime<Utc>) -> bool {
        self.not_before <= instant && instant <= self.not_after
    }
}

/// `EnvelopeHandle` — an opaque, serializable reference to a sealed
/// `ExecutionEnvelope` (id + content hash). Unlike `ExecutionEnvelope`
/// itself, `EnvelopeHandle` IS serializable (§9.3: "the runtime accepts
/// only `ExecutionEnvelope`, not raw agent output" — a handle is how a
/// *reference* to an already-sealed envelope crosses a persistence or wire
/// boundary; T4.2 persists handles in `control_plane_envelopes` and
/// rehydrates only through control-plane re-verification, never by
/// deserializing a raw envelope).
///
/// T8.1 (EOP-PLAN-CONTROLPLANE-001, closes PIR-D-008/PIR-D-010): the type
/// itself now lives in `ob-poc-types::envelope_handle` — a values-only
/// boundary crate `dsl-runtime` can depend on without pulling in this
/// crate's gate logic — so `VerbExecutionPort::execute_verb_admitting_envelope`
/// can carry a typed, content-hash-checked handle instead of a bare `Uuid`.
/// Re-exported here so existing `ob_poc_control_plane::envelope::EnvelopeHandle`
/// call sites are unaffected.
pub use ob_poc_types::EnvelopeHandle;

/// `ExecutionEnvelope` — the sealed, runtime-admissible artefact. Private
/// fields, no public constructor, and deliberately **no `Deserialize`**
/// (see the trybuild fixture proving this): the runtime must obtain an
/// envelope only via `seal`, never by deserializing one from storage or the
/// wire (that path exists only for `EnvelopeHandle`, and even then only
/// through control-plane re-verification per §6.10.4).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionEnvelope {
    id: Uuid,
    intent: AdmittedIntent,
    binding: BoundEntities,
    pack: ResolvedPack,
    dag: LegalTransition,
    authority: Authorised,
    evidence: EvidenceSufficient,
    write_set: WriteSetProof,
    runbook: CompiledRunbookRef,
    snapshot: SnapshotPins,
    validity: ValidityWindow,
}

impl ExecutionEnvelope {
    /// The only constructor. `pub(crate)`: unreachable from outside this
    /// crate (see `tests/trybuild/seal_is_crate_private.rs`). Requires
    /// every gate's success proof by signature, exactly per V&S §9.4 —
    /// adding a gate to the platform means adding a parameter here;
    /// forgetting to run it becomes unrepresentable rather than undetected.
    ///
    /// Called by the (future) full-orchestration `evaluate()` once every
    /// gate has a real adapter; today's callers are the cfg(test)
    /// positive-path test below and, behind the `test-support` feature,
    /// `envelope::test_support::seal` (for downstream crates' storage-layer
    /// tests — see that module's doc).
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn seal(
        intent: AdmittedIntent,
        binding: BoundEntities,
        pack: ResolvedPack,
        dag: LegalTransition,
        authority: Authorised,
        evidence: EvidenceSufficient,
        write_set: WriteSetProof,
        runbook: CompiledRunbookRef,
        snapshot: SnapshotPins,
        validity: ValidityWindow,
    ) -> ExecutionEnvelope {
        ExecutionEnvelope {
            id: Uuid::new_v4(),
            intent,
            binding,
            pack,
            dag,
            authority,
            evidence,
            write_set,
            runbook,
            snapshot,
            validity,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn intent(&self) -> &AdmittedIntent {
        &self.intent
    }

    pub fn binding(&self) -> &BoundEntities {
        &self.binding
    }

    pub fn pack(&self) -> &ResolvedPack {
        &self.pack
    }

    pub fn dag(&self) -> &LegalTransition {
        &self.dag
    }

    pub fn authority(&self) -> &Authorised {
        &self.authority
    }

    pub fn evidence(&self) -> &EvidenceSufficient {
        &self.evidence
    }

    pub fn write_set(&self) -> &WriteSetProof {
        &self.write_set
    }

    pub fn runbook(&self) -> &CompiledRunbookRef {
        &self.runbook
    }

    pub fn snapshot(&self) -> &SnapshotPins {
        &self.snapshot
    }

    pub fn validity(&self) -> ValidityWindow {
        self.validity
    }

    /// Mint an opaque `EnvelopeHandle` referencing this sealed envelope.
    pub fn handle(&self) -> EnvelopeHandle {
        let content = serde_json::to_vec(self).expect("ExecutionEnvelope always serializes");
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let content_hash: [u8; 32] = hasher.finalize().into();
        EnvelopeHandle::new(self.id, content_hash)
    }

    /// T10.1 (EOP-PLAN-CONTROLPLANE-001 Addendum C, B2): the single
    /// sanctioned way for a sealed envelope's content to leave this crate.
    /// `EnvelopeRecord` is a flattened, primitive-typed **projection**, not
    /// a serde mirror of `ExecutionEnvelope` — none of the individual proof
    /// types (`AdmittedIntent`, `BoundEntities`, `Authorised`, ...) gain
    /// `Deserialize` here or anywhere else; only their already-plain-data
    /// fields are copied out. This is deliberate: a `EnvelopeRecord` read
    /// back from storage can be inspected (T10.2's pin comparison, future
    /// audit) but can never be fed back into a `seal()`-equivalent
    /// constructor to fabricate a proof — "rehydration of a record into
    /// anything execution-accepted happens only through control-plane
    /// verification" (§6.10.4), i.e. a future comparison function taking
    /// `&EnvelopeRecord` plus freshly-read facts, never a raw deserialize
    /// into `ExecutionEnvelope`.
    pub fn to_record(&self) -> EnvelopeRecord {
        EnvelopeRecord {
            envelope_id: self.id,
            verb_fqn: self.intent.verb_fqn().to_string(),
            bound_entity_ids: self.binding.entity_ids().to_vec(),
            pack_id: self.pack.pack_id().to_string(),
            snapshot: self.snapshot.clone(),
            not_before: self.validity.not_before(),
            not_after: self.validity.not_after(),
        }
    }
}

/// T10.1: the flattened, storable projection of a sealed `ExecutionEnvelope`
/// — see [`ExecutionEnvelope::to_record`] for why this is a distinct type
/// rather than a `Deserialize` derive on the envelope or its proof types.
/// Every field here is already plain data (`Uuid`/`String`/`Vec`/pins),
/// copied out of proof types whose own constructors stay module-private.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnvelopeRecord {
    pub envelope_id: Uuid,
    pub verb_fqn: String,
    pub bound_entity_ids: Vec<Uuid>,
    pub pack_id: String,
    pub snapshot: crate::snapshot::SnapshotPins,
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
}

/// Feature-gated (`test-support`, off by default), NOT `cfg(test)`: a
/// downstream crate's own `cargo test` run does not compile this crate
/// with `cfg(test)` active (that only applies to a crate's *own* test
/// target), so a `cfg(test)` bridge here would be invisible to e.g.
/// `ob-poc`'s T4.2 envelope-persistence tests. This module exists solely so
/// those tests can obtain a real, fully-proven `ExecutionEnvelope` to
/// exercise storage/consume-once/TTL logic against — every parameter is
/// still each gate's own module-private success proof, unchanged from
/// `seal` itself.
#[cfg(feature = "test-support")]
pub mod test_support {
    use super::{
        AdmittedIntent, Authorised, BoundEntities, CompiledRunbookRef, EvidenceSufficient, ExecutionEnvelope,
        LegalTransition, ResolvedPack, SnapshotPins, ValidityWindow, WriteSetProof,
    };

    #[allow(clippy::too_many_arguments)]
    pub fn seal(
        intent: AdmittedIntent,
        binding: BoundEntities,
        pack: ResolvedPack,
        dag: LegalTransition,
        authority: Authorised,
        evidence: EvidenceSufficient,
        write_set: WriteSetProof,
        runbook: CompiledRunbookRef,
        snapshot: SnapshotPins,
        validity: ValidityWindow,
    ) -> ExecutionEnvelope {
        ExecutionEnvelope::seal(
            intent, binding, pack, dag, authority, evidence, write_set, runbook, snapshot, validity,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn now_window() -> ValidityWindow {
        let now = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        ValidityWindow::new(now, now + chrono::Duration::minutes(5))
    }

    /// Positive-path proof: with every success-form proof in hand (all
    /// constructed here, inside the crate, via each module's own
    /// internals), `seal` produces a real envelope and a stable handle.
    /// This is the compile-time contrast to the trybuild fixtures proving
    /// the negative path is unreachable.
    #[test]
    fn seal_succeeds_given_every_success_proof() {
        // Each proof type's constructor is private to its own module, so
        // this test exercises the crate-internal round-trip via the same
        // access every (future) real gate adapter will use once T2 lands:
        // module-internal construction, success value flows to `seal`.
        let intent = crate::intent_admission::tests_support::admitted(Uuid::nil(), "cbu.confirm");
        let binding = crate::entity_binding::tests_support::bound(vec![Uuid::nil()]);
        let pack = crate::pack_resolution::tests_support::resolved("ob-poc.cbu");
        let dag = crate::dag_proof::tests_support::legal(
            Uuid::nil(),
            "VALIDATION_PENDING",
            "VALIDATED",
        );
        let authority = crate::authority_gate::tests_support::authorised("actor-1", "compliance_officer");
        let evidence = crate::evidence_gate::tests_support::sufficient(vec!["obligation-1".into()]);
        let write_set = crate::write_set::tests_support::proof(
            vec![Uuid::nil()],
            vec!["validation_state".into()],
            vec!["ob-poc.cbus".into()],
            vec!["status".into()],
            "idem-1",
        );
        let runbook = CompiledRunbookRef::new(Uuid::nil());
        let snapshot = crate::snapshot::tests_support::pins(Some(Uuid::nil()), None, None, vec![]);

        let envelope = ExecutionEnvelope::seal(
            intent,
            binding,
            pack,
            dag,
            authority,
            evidence,
            write_set,
            runbook,
            snapshot,
            now_window(),
        );

        let handle = envelope.handle();
        assert_eq!(handle.id(), envelope.id());
        assert_eq!(handle.content_hash_hex().len(), 64);
    }

    /// T10.1: `to_record()` copies out plain data only — the record must
    /// round-trip through JSON (proving it's genuinely storable) while the
    /// envelope's own proof fields stay private/non-`Deserialize`.
    #[test]
    fn to_record_copies_out_plain_data_and_round_trips_through_json() {
        let intent = crate::intent_admission::tests_support::admitted(Uuid::nil(), "cbu.confirm");
        let binding = crate::entity_binding::tests_support::bound(vec![Uuid::nil()]);
        let pack = crate::pack_resolution::tests_support::resolved("ob-poc.cbu");
        let dag = crate::dag_proof::tests_support::legal(Uuid::nil(), "VALIDATION_PENDING", "VALIDATED");
        let authority = crate::authority_gate::tests_support::authorised("actor-1", "compliance_officer");
        let evidence = crate::evidence_gate::tests_support::sufficient(vec!["obligation-1".into()]);
        let write_set = crate::write_set::tests_support::proof(
            vec![Uuid::nil()],
            vec!["validation_state".into()],
            vec!["ob-poc.cbus".into()],
            vec!["status".into()],
            "idem-1",
        );
        let runbook = CompiledRunbookRef::new(Uuid::nil());
        let snapshot = crate::snapshot::tests_support::pins(
            Some(Uuid::nil()),
            None,
            None,
            vec![(Uuid::nil(), "cbu".to_string(), 3)],
        );

        let envelope = ExecutionEnvelope::seal(
            intent, binding, pack, dag, authority, evidence, write_set, runbook, snapshot,
            now_window(),
        );

        let record = envelope.to_record();
        assert_eq!(record.envelope_id, envelope.id());
        assert_eq!(record.verb_fqn, "cbu.confirm");
        assert_eq!(record.bound_entity_ids, vec![Uuid::nil()]);
        assert_eq!(record.pack_id, "ob-poc.cbu");
        assert_eq!(record.snapshot.entity_row_version(Uuid::nil()), Some(3));
        assert_eq!(record.not_before, now_window().not_before());

        let json = serde_json::to_string(&record).expect("EnvelopeRecord serializes");
        let round_tripped: EnvelopeRecord =
            serde_json::from_str(&json).expect("EnvelopeRecord deserializes");
        assert_eq!(round_tripped, record);
    }

    #[test]
    fn validity_window_contains_checks_bounds_inclusively() {
        let start = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let end = start + chrono::Duration::minutes(1);
        let window = ValidityWindow::new(start, end);
        assert!(window.contains(start));
        assert!(window.contains(end));
        assert!(!window.contains(end + chrono::Duration::seconds(1)));
    }

    #[test]
    #[should_panic(expected = "not_before must precede not_after")]
    fn validity_window_rejects_inverted_bounds() {
        let start = DateTime::<Utc>::from_timestamp(100, 0).unwrap();
        let end = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        ValidityWindow::new(start, end);
    }
}

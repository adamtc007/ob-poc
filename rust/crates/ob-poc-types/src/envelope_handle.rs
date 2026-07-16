//! `EnvelopeHandle` — the opaque, serializable reference to a sealed
//! `ob_poc_control_plane::envelope::ExecutionEnvelope` (EOP-PLAN-CONTROLPLANE-001
//! T8.1, closes PIR-D-008/PIR-D-010).
//!
//! Lives here, not in `ob-poc-control-plane`, so `dsl-runtime`'s
//! `VerbExecutionPort::execute_verb_admitting_envelope` can carry a typed
//! handle (id + content hash) across the trait boundary without depending
//! on the control-plane crate — `dsl-runtime` is the pure execution-tier
//! contract crate and must not know about gate logic (see that trait
//! method's doc comment). `ob-poc-types` is a values-only boundary crate
//! both `dsl-runtime` and `ob-poc-control-plane` can depend on without
//! either depending on the other.
//!
//! Deliberately a plain, fully-public value type: `EnvelopeHandle` itself
//! carries no authority. The actual admission check is the DB-backed
//! `try_consume` (`ob-poc::agent::control_plane_envelope_store`) verifying
//! this handle's `content_hash` against the persisted row minted by
//! `ExecutionEnvelope::seal`'s caller — anyone constructing an
//! `EnvelopeHandle` with a guessed id/hash gets rejected there, exactly as
//! before this type moved out of `ob-poc-control-plane`'s crate boundary.

use uuid::Uuid;

/// An opaque, serializable reference to a sealed `ExecutionEnvelope`
/// (id + content hash). See module doc for why this is a plain value type
/// here rather than a control-plane-owned type with a restricted
/// constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EnvelopeHandle {
    id: Uuid,
    /// SHA-256 of the envelope's sealed content, 32 raw bytes.
    content_hash: [u8; 32],
}

impl EnvelopeHandle {
    pub fn new(id: Uuid, content_hash: [u8; 32]) -> Self {
        Self { id, content_hash }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn content_hash(&self) -> [u8; 32] {
        self.content_hash
    }

    pub fn content_hash_hex(&self) -> String {
        hex::encode(self.content_hash)
    }
}

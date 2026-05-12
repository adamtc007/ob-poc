//! Phase A (F5) — shadow envelope construction.
//!
//! Three-plane v0.3 §10.3 says the bi-plane boundary between SemOS and the
//! DSL runtime is a [`GatedVerbEnvelope`] value. Pre-Phase-A, the envelope
//! types were compile-only — stage 6 ("gate decision") produced no envelope,
//! and the dispatch path operated on scattered args rather than a single
//! boundary value. This module closes the gap by building a **shadow
//! envelope** at each dispatch opportunity.
//!
//! # What "shadow" means
//!
//! The envelope is constructed alongside the existing dispatch path and
//! emitted as a `tracing::debug!` event with structured fields. It does NOT
//! gate execution yet — the data plane continues to consume `VerbCall` /
//! args as before. Phase B (F6) is the slice that hoists transaction
//! ownership into the Sequencer and makes the envelope the *primary*
//! dispatch contract.
//!
//! # Why shadow first
//!
//! Structural refactor of the dispatch path requires:
//!   1. every field of the envelope to be populated with real data, and
//!   2. every consumer to migrate to envelope-based signatures.
//!
//! Shadow emission unblocks (1) incrementally: each field moves from
//! `<phase_a_todo>` placeholder to real data in its own sub-slice, and the
//! determinism harness can byte-compare the envelope across runs **before**
//! it becomes load-bearing. Once every field is real, Phase B flips the
//! contract.
//!
//! # Placeholder fields
//!
//! Several envelope fields have no live source in the current orchestrator:
//!
//! - [`CatalogueSnapshotId`] — SemOS catalogue revision. Today the
//!   `SemOsContextEnvelope.fingerprint` is the closest proxy; Phase A later
//!   adds a first-class `sem_reg.snapshots` revision column and plumbs it.
//! - [`DagNodeId`] / [`DagNodeVersion`] — which constellation node the
//!   invocation targets. Today the session carries `active_workspace` but no
//!   per-entity node cursor; a Phase D prerequisite.
//! - [`StateGateHash`] — TOCTOU fingerprint. Requires row-versioning
//!   (Phase D / F9). This module emits the zero-hash placeholder for now.
//! - [`ResolvedEntities`] — today's scope resolver returns a `Vec<Uuid>` of
//!   CBU ids; this module wraps them in `ResolvedEntity` with empty state
//!   snapshots. Phase A-2 will add `(entity_id, row_version)` pairs once
//!   migrations land.
//!
//! Each placeholder is tagged `<phase_a_todo>` or `<phase_d_todo>` in the
//! builder so the determinism harness can detect when a placeholder is
//! replaced by real data.

use ob_poc_types::gated_envelope::state_gate_hash;
use ob_poc_types::{
    AuthorisationProof, CatalogueSnapshotId, ClosedLoopMarker, DagNodeId, DagNodeVersion,
    DiscoverySignals, EnvelopeVersion, GatedVerbEnvelope, LogicalClock, ResolvedEntities,
    SessionScopeRef, TraceId, VerbArgs, VerbRef, WorkspaceSnapshotId,
};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Inputs the Sequencer has at stage 6 — what the dispatch site already
/// knows before it hands off to the runtime. Everything we can't yet
/// populate is captured as an explicit placeholder.
pub struct EnvelopeInputs<'a> {
    pub session_id: Uuid,
    /// Canonical verb FQN — e.g. `"cbu.create"`.
    pub verb_fqn: &'a str,
    /// Args resolved against the session's symbol table, as JSON. Phase B
    /// will replace this with a typed arg shape per verb contract.
    pub args: JsonValue,
    /// `writes_since_push` on the session at the moment of gating.
    pub writes_since_push_at_gate: u64,
    /// CBU UUIDs currently in scope. Phase A-2 will pair each with a
    /// `row_version` once migrations exist.
    pub scope_cbu_ids: &'a [Uuid],
    /// SemReg allowed-verbs fingerprint from the live envelope. Used as a
    /// proxy for [`CatalogueSnapshotId`] until a first-class catalogue
    /// revision column lands.
    pub semreg_fingerprint_proxy: Option<&'a str>,
    /// Logical clock value at gate time. Today: monotonic counter derived
    /// from the session's trace sequence. Phase D: replaced by a proper
    /// SemOS-issued logical clock.
    pub logical_clock: u64,
    /// Phase A.2 (F5 follow-on, 2026-04-22): explicit turn-level trace_id
    /// from the session. When `Some`, the envelope carries this id —
    /// enabling correlation between the `ReplResponseV2.trace_id` seen by
    /// the frontend and the `GatedVerbEnvelope.trace_id` in runtime logs.
    /// When `None`, a fresh id is generated (legacy behaviour).
    pub trace_id: Option<Uuid>,
}

/// Build a shadow envelope from the dispatch site's currently-available
/// data. Placeholder fields are clearly marked so the determinism harness
/// can distinguish "real data not yet plumbed" from "genuine value".
pub fn build_shadow_envelope(inputs: &EnvelopeInputs<'_>) -> GatedVerbEnvelope {
    let trace_id = TraceId(inputs.trace_id.unwrap_or_else(Uuid::new_v4));
    let session_scope = SessionScopeRef(inputs.session_id);

    // <phase_a_todo> CatalogueSnapshotId: derive from sem_reg snapshot
    // revision column once the schema addition lands.  For now project
    // the SemReg allowed-verbs fingerprint into a `u64` via stable hash
    // so the field isn't zero.
    let catalogue_snapshot_id = inputs
        .semreg_fingerprint_proxy
        .map(fingerprint_to_snapshot_id)
        .unwrap_or(CatalogueSnapshotId(0));

    // <phase_a_todo> Resolved entities: attach real row_version + state
    // snapshot references when Phase D row-versioning lands. For now the
    // state snapshot ref is empty and row_version is 0.
    let mut entity_list: Vec<ob_poc_types::ResolvedEntity> = inputs
        .scope_cbu_ids
        .iter()
        .map(|cbu_id| ob_poc_types::ResolvedEntity {
            entity_id: *cbu_id,
            entity_kind: "cbu".into(),
            row_version: 0,
        })
        .collect();
    // Spec §10.5 canonical encoding requires entity order by entity_id so
    // the StateGateHash is deterministic. Sort here so the shadow envelope
    // matches the future real encoding bit-for-bit.
    entity_list.sort_by_key(|e| e.entity_id);
    let resolved_entities = ResolvedEntities(entity_list);

    // <phase_a_todo> DagNodeId / DagNodeVersion: today no per-entity DAG
    // cursor exists on the session. Placeholder = session_id as node id,
    // version 0.
    let dag_position = DagNodeId(inputs.session_id);
    let dag_node_version = DagNodeVersion(0);

    // Phase A.4 (Phase D.3 partial, 2026-04-22): compute the real BLAKE3
    // StateGateHash over the spec §10.5 canonical encoding. The hash is
    // deterministic given the same (entities, dag position, scope,
    // workspace snapshot, catalogue snapshot) — when row_version columns
    // populate from Phase D.2 backfill, the hash becomes a genuine
    // TOCTOU fingerprint. Pre-Phase-D.2, row_version fields are still 0
    // (placeholder in ResolvedEntity above), so the hash is still
    // determined by (entity_ids, ids, scope) — but it's no longer the
    // zero sentinel. Slice D.3 wires the runtime-side recheck that
    // compares this hash against a post-lock recomputation.
    //
    // WorkspaceSnapshotId: <phase_a_todo> today the session has no
    // workspace-level snapshot cursor. Placeholder: session_id as the
    // workspace snapshot id so the hash is still deterministic per
    // session. Phase D.2 backfills real workspace snapshot ids from
    // constellation rehydration events.
    let workspace_snapshot_id = WorkspaceSnapshotId(inputs.session_id);
    let state_gate_hash = state_gate_hash::hash(
        EnvelopeVersion::CURRENT,
        &resolved_entities,
        dag_position,
        dag_node_version,
        session_scope,
        workspace_snapshot_id,
        catalogue_snapshot_id,
    );

    let authorisation = AuthorisationProof {
        issued_at: LogicalClock(inputs.logical_clock),
        session_scope,
        state_gate_hash,
        // <phase_d_todo> recheck_required flips to `true` when Phase D
        // TOCTOU recheck is wired inside the dispatch transaction.
        recheck_required: false,
    };

    GatedVerbEnvelope {
        envelope_version: EnvelopeVersion::CURRENT,
        catalogue_snapshot_id,
        trace_id,
        verb: VerbRef(inputs.verb_fqn.to_string()),
        dag_position,
        dag_node_version,
        resolved_entities,
        args: VerbArgs(inputs.args.clone()),
        authorisation,
        discovery_signals: DiscoverySignals::default(),
        closed_loop_marker: ClosedLoopMarker {
            writes_since_push_at_gate: inputs.writes_since_push_at_gate,
        },
    }
}

/// Project a SemReg fingerprint string (e.g. `"v1:ab12…"`) into a stable
/// `CatalogueSnapshotId`. Two fingerprints with identical content produce
/// the same id; different fingerprints produce different ids with extremely
/// high probability.
fn fingerprint_to_snapshot_id(fingerprint: &str) -> CatalogueSnapshotId {
    // Slice a u64 out of the fingerprint's hex portion. Fingerprints are
    // `v1:<64-hex>` (SHA-256 truncated), so take the first 16 hex chars.
    let hex = fingerprint.trim_start_matches("v1:");
    let chunk: String = hex.chars().take(16).collect();
    let as_u64 = u64::from_str_radix(&chunk, 16).unwrap_or(0);
    CatalogueSnapshotId(as_u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_envelope_populates_all_required_fields() {
        let session_id = Uuid::new_v4();
        let cbus = [Uuid::new_v4(), Uuid::new_v4()];
        let inputs = EnvelopeInputs {
            session_id,
            verb_fqn: "cbu.create",
            args: serde_json::json!({"name": "Test Fund"}),
            writes_since_push_at_gate: 7,
            scope_cbu_ids: &cbus,
            semreg_fingerprint_proxy: Some("v1:deadbeef12345678"),
            logical_clock: 42,
            trace_id: None,
        };
        let env = build_shadow_envelope(&inputs);

        assert_eq!(env.envelope_version, EnvelopeVersion::CURRENT);
        assert_eq!(env.verb.0, "cbu.create");
        assert_eq!(env.resolved_entities.0.len(), 2);
        assert_eq!(env.authorisation.session_scope.0, session_id);
        assert_eq!(env.authorisation.issued_at.0, 42);
        assert_eq!(env.closed_loop_marker.writes_since_push_at_gate, 7);
        // Catalogue snapshot id should be non-zero when a fingerprint is supplied.
        assert_ne!(env.catalogue_snapshot_id.0, 0);
        // Phase A.4: StateGateHash is now the real BLAKE3 hash, not the
        // zero sentinel. Any non-zero hash value is acceptable — Phase
        // D.3's recheck asserts equality against a post-lock
        // recomputation, not a specific byte pattern.
        assert_ne!(env.authorisation.state_gate_hash.0, [0u8; 32]);
    }

    #[test]
    fn shadow_envelope_state_gate_hash_is_deterministic() {
        // Phase A.4 regression: the same inputs produce the same
        // StateGateHash across runs. This is the canonical-encoding
        // determinism obligation (spec §9.1 + §10.5).
        let session_id = Uuid::from_u128(0xdead_beef_cafe_babe_0000_0000_0000_0001);
        let cbus = [
            Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001),
            Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0002),
        ];
        let mk = || EnvelopeInputs {
            session_id,
            verb_fqn: "cbu.create",
            args: serde_json::json!({"name": "Deterministic"}),
            writes_since_push_at_gate: 0,
            scope_cbu_ids: &cbus,
            semreg_fingerprint_proxy: Some("v1:0011223344556677"),
            logical_clock: 1,
            trace_id: Some(Uuid::nil()),
        };
        let h1 = build_shadow_envelope(&mk()).authorisation.state_gate_hash;
        let h2 = build_shadow_envelope(&mk()).authorisation.state_gate_hash;
        assert_eq!(
            h1, h2,
            "StateGateHash must be deterministic over identical inputs"
        );

        // Any input change produces a different hash.
        let different_session = Uuid::from_u128(0xdead_beef_cafe_babe_0000_0000_0000_0002);
        let h3 = build_shadow_envelope(&EnvelopeInputs {
            session_id: different_session,
            ..mk()
        })
        .authorisation
        .state_gate_hash;
        assert_ne!(
            h1, h3,
            "Different session_id must produce a different StateGateHash"
        );
    }

    #[test]
    fn shadow_envelope_with_no_fingerprint_has_zero_snapshot() {
        let inputs = EnvelopeInputs {
            session_id: Uuid::nil(),
            verb_fqn: "entity.ensure",
            args: serde_json::Value::Null,
            writes_since_push_at_gate: 0,
            scope_cbu_ids: &[],
            semreg_fingerprint_proxy: None,
            logical_clock: 0,
            trace_id: None,
        };
        let env = build_shadow_envelope(&inputs);
        assert_eq!(env.catalogue_snapshot_id.0, 0);
        assert!(env.resolved_entities.0.is_empty());
    }

    #[test]
    fn shadow_envelope_reuses_provided_trace_id() {
        // Phase A.2 regression: when the caller supplies a trace_id from
        // the session, the envelope must carry THAT id — not a fresh one.
        // This is what makes the frontend-visible `ReplResponseV2.trace_id`
        // and runtime-log `GatedVerbEnvelope.trace_id` the same id, so one
        // grep threads an utterance from stage 1 through stage 9b.
        let pinned = Uuid::new_v4();
        let inputs = EnvelopeInputs {
            session_id: Uuid::nil(),
            verb_fqn: "cbu.create",
            args: serde_json::Value::Null,
            writes_since_push_at_gate: 0,
            scope_cbu_ids: &[],
            semreg_fingerprint_proxy: None,
            logical_clock: 0,
            trace_id: Some(pinned),
        };
        let env = build_shadow_envelope(&inputs);
        assert_eq!(env.trace_id.0, pinned);

        // And when None is supplied, a fresh UUID is generated.
        let inputs_no_id = EnvelopeInputs {
            trace_id: None,
            ..inputs
        };
        let env_fresh = build_shadow_envelope(&inputs_no_id);
        assert_ne!(env_fresh.trace_id.0, pinned);
    }

    #[test]
    fn fingerprint_projection_is_stable() {
        let a = fingerprint_to_snapshot_id("v1:deadbeef12345678abcdef");
        let b = fingerprint_to_snapshot_id("v1:deadbeef12345678zzzz");
        // Same first 16 hex chars → identical projections.
        assert_eq!(a.0, b.0);
        let c = fingerprint_to_snapshot_id("v1:0011223344556677");
        assert_ne!(a.0, c.0);
    }
}

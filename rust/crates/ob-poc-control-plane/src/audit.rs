//! G11 — Audit and Replay Record (V&S §6.11).
//!
//! `T7.1` / `EOP-DESIGN-CONTROLPLANE-G2-AUDIT-PROVENANCE-001` (v0.2,
//! RATIFIED) §2-§4: the typed, exhaustively-matched `AuditEvent` enum that
//! is the Rust-side counterpart of `"ob-poc".control_plane_audit`
//! (migration `20260713_control_plane_audit.sql`), plus the `GateOutcomeProvenance`
//! dimension (§3, DD-3) and the per-gate expected-provenance map (§3,
//! normative for the E3 probe).
//!
//! Persistence (the actual `INSERT`/`SELECT` against `control_plane_audit`)
//! lives in the `ob-poc` root crate's `agent::control_plane_audit` module,
//! behind its own `database` feature -- this crate stays free of any DB
//! dependency (§9.1 non-goal), same posture as every other module here.
//! `AuditEvent` itself carries zero DB types; it is a pure value + a
//! `serde_json`-based (de)serialization boundary so a persistence layer
//! elsewhere can store/reload it without this crate knowing sqlx exists.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// The three-way summary of a `ControlPlaneDecision` (§9.3's richer boxed
/// variants carry proof-bearing values not appropriate for an audit
/// payload -- this is the outcome *tag* only, per DD-2's `AuditEvent`
/// sketch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DecisionOutcome {
    ApprovedStp,
    HumanGate,
    Rejected,
}

/// DD-3: provenance is a closed three-value enum, stored implicitly by
/// event locus (`EnvelopeConsumed` => `ConsumeSeam`, `DispatchCommitted`
/// => `PostDispatch`; `control_plane_shadow_decisions` rows are
/// `ShadowEval` by definition, no column needed), materialized explicitly
/// only in the rebuilt counting view (`agent::control_plane_metrics::gate_outcome_counts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GateOutcomeProvenance {
    ShadowEval,
    ConsumeSeam,
    PostDispatch,
}

impl GateOutcomeProvenance {
    /// The literal string used in the rebuilt `gate_outcome_counts` UNION
    /// (§3's query sketch: `'shadow_eval'` / `'consume_seam'` / `'post_dispatch'`).
    pub fn as_str(&self) -> &'static str {
        match self {
            GateOutcomeProvenance::ShadowEval => "shadow_eval",
            GateOutcomeProvenance::ConsumeSeam => "consume_seam",
            GateOutcomeProvenance::PostDispatch => "post_dispatch",
        }
    }
}

/// §3's per-gate expected-provenance map, normative for the E3 probe:
/// "G1-G9, G12, G13 -> ShadowEval; G10 -> ConsumeSeam (AD-1(a)); G14 ->
/// PostDispatch; G11 -> ShadowEval over the audit stream itself (§4)."
///
/// Exhaustively matched, no `_` arm -- same doctrine as `GateId` /
/// `GateResult` elsewhere in this crate: a 15th `GateId` variant must break
/// this at compile time until the map is extended (§3: "G15+ additions
/// must extend this map or fail the exhaustiveness test").
pub fn expected_provenance(gate: crate::gate::GateId) -> GateOutcomeProvenance {
    use crate::gate::GateId;
    match gate {
        GateId::IntentAdmission => GateOutcomeProvenance::ShadowEval,
        GateId::EntityBinding => GateOutcomeProvenance::ShadowEval,
        GateId::PackResolution => GateOutcomeProvenance::ShadowEval,
        GateId::DagProof => GateOutcomeProvenance::ShadowEval,
        GateId::Authority => GateOutcomeProvenance::ShadowEval,
        GateId::Evidence => GateOutcomeProvenance::ShadowEval,
        GateId::WriteSet => GateOutcomeProvenance::ShadowEval,
        GateId::StpClassifier => GateOutcomeProvenance::ShadowEval,
        GateId::RunbookProof => GateOutcomeProvenance::ShadowEval,
        // G10 (ExecutionEnvelope): AD-1(a), graded at consume time.
        GateId::ExecutionEnvelope => GateOutcomeProvenance::ConsumeSeam,
        // G11 (AuditReplay): ShadowEval-shaped -- it evaluates *over* the
        // audit stream, but its own samples are recorded the same way
        // every other shadow-time gate's are (§3, §4).
        GateId::AuditReplay => GateOutcomeProvenance::ShadowEval,
        GateId::VersionPinning => GateOutcomeProvenance::ShadowEval,
        GateId::DecisionSnapshot => GateOutcomeProvenance::ShadowEval,
        // G14 (WriteSetAttestation): graded post-dispatch, at commit time.
        GateId::WriteSetAttestation => GateOutcomeProvenance::PostDispatch,
    }
}

/// One gate's outcome, recorded on a later-arriving audit event
/// (`EnvelopeConsumed`'s G10 grading, `DispatchCommitted`'s G14 grading).
/// `gate` is the `GateId`'s `Debug` rendering (e.g. `"ExecutionEnvelope"`)
/// -- matches the existing `gate_results` JSONB convention
/// (`agent::control_plane_shadow::report_to_json`) so the rebuilt
/// `gate_outcome_counts` query can UNION both sources by the same key
/// shape without a translation table. `outcome_kind` matches the same
/// existing classifier vocabulary (`"Success"` / `"Failure"` /
/// `"NotEvaluated"` / `"NotImplemented"`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GateOutcomeRecord {
    pub gate: String,
    pub outcome_kind: String,
}

impl GateOutcomeRecord {
    pub fn new(gate: crate::gate::GateId, outcome_kind: impl Into<String>) -> Self {
        Self {
            gate: format!("{gate:?}"),
            outcome_kind: outcome_kind.into(),
        }
    }
}

/// DD-2: the exhaustively-matched, append-only lifecycle event enum.
/// "Exhaustively matched everywhere. Adding a variant must break every
/// consumer until handled -- same doctrine as `GateId` / `GateResult`."
///
/// Serialized externally-tagged (`serde`'s default enum representation):
/// `{"DecisionEvaluated": {...}}`. `event_type()` returns the tag string
/// used both as the column value and as the JSON object key wrapping the
/// payload -- `payload_json()`/`from_stored()` use this equivalence to
/// split/reassemble the `event_type` column and `payload` JSONB column
/// from a single derived `Serialize`/`Deserialize` impl, rather than
/// hand-rolling per-variant JSON construction (CLAUDE.md's "never
/// untyped JSON for structured data" -- the payload IS a typed struct,
/// this is a boundary conversion, not ad hoc construction).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuditEvent {
    /// Emitted where the shadow row is written today (`phase5_runtime_recheck`),
    /// as an additional insert in the same scope. W1: this insert must not
    /// alter what the shadow row itself records.
    DecisionEvaluated {
        outcome: DecisionOutcome,
        /// Best-effort informational reference into `SnapshotInput`'s own
        /// `sem_reg_snapshot_id` (V2 finding: no persisted, queryable
        /// `SnapshotId` store exists for G13 -- see the implementing
        /// session doc). Not a join key to any table; `None` when no
        /// snapshot input was collected.
        snapshot_ref: Option<Uuid>,
        /// G2 item 3 (G11 wiring, `EOP-SESSION-CONTROLPLANE-G2-ITEMS-2-3-
        /// CLOSURE-001`): the shadow entry this decision was evaluated
        /// for -- `control_plane_shadow_decisions.entry_id`, the SAME
        /// value `build_shadow_decision_row` is called with at the one
        /// real emission site (`sequencer.rs::phase5_runtime_recheck`).
        /// Needed so the G11 replay surface can join back to that row's
        /// `gate_results` for DD-4(ii) outcome re-derivation -- nothing
        /// else persisted on this event carries a stable link to it.
        /// `#[serde(default)]` (defaults to `Uuid::nil()`, matching
        /// `Uuid::default()`): any already-persisted `DecisionEvaluated`
        /// row from before this field existed still deserializes; the
        /// replay surface treats a nil `entry_id` as "no re-derivation
        /// join available for this row" (see `audit_replay_outcome_counts`),
        /// not a crash.
        #[serde(default)]
        entry_id: Uuid,
    },
    EnvelopeSealed {
        envelope_id: Uuid,
        expires_at: DateTime<Utc>,
    },
    /// G10, provenance `ConsumeSeam` (AD-1(a)).
    EnvelopeConsumed {
        envelope_id: Uuid,
        gate_outcome: GateOutcomeRecord,
    },
    /// G14, provenance `PostDispatch`.
    DispatchCommitted {
        attested: bool,
        gate_outcome: GateOutcomeRecord,
    },
    DispatchRolledBack {
        reason: String,
    },
    /// Not wired to any production emission site by this doc (standing
    /// rule 3 -- divergence classification logic itself is untouched); the
    /// variant exists so the type is exhaustively representable per DD-2,
    /// and future divergence-triage tooling has a typed home to write to.
    DivergenceTriaged {
        classification: String,
        runbook_ref: String,
    },
}

impl AuditEvent {
    /// The serialized discriminant -- both the `event_type` column value
    /// and the external-tag JSON key `payload_json()`/`from_stored()` use
    /// to split/reassemble the payload.
    pub fn event_type(&self) -> &'static str {
        match self {
            AuditEvent::DecisionEvaluated { .. } => "DecisionEvaluated",
            AuditEvent::EnvelopeSealed { .. } => "EnvelopeSealed",
            AuditEvent::EnvelopeConsumed { .. } => "EnvelopeConsumed",
            AuditEvent::DispatchCommitted { .. } => "DispatchCommitted",
            AuditEvent::DispatchRolledBack { .. } => "DispatchRolledBack",
            AuditEvent::DivergenceTriaged { .. } => "DivergenceTriaged",
        }
    }

    /// §3: the provenance an event's embedded `GateOutcomeRecord`
    /// contributes at, when it carries one. `None` for events that don't
    /// carry a gate outcome at all (`DecisionEvaluated`'s gates are
    /// `ShadowEval` by the shadow-row's own definition, not via this
    /// audit event).
    pub fn provenance(&self) -> Option<GateOutcomeProvenance> {
        match self {
            AuditEvent::EnvelopeConsumed { .. } => Some(GateOutcomeProvenance::ConsumeSeam),
            AuditEvent::DispatchCommitted { .. } => Some(GateOutcomeProvenance::PostDispatch),
            AuditEvent::DecisionEvaluated { .. }
            | AuditEvent::EnvelopeSealed { .. }
            | AuditEvent::DispatchRolledBack { .. }
            | AuditEvent::DivergenceTriaged { .. } => None,
        }
    }

    /// The `payload` JSONB column value: the event's own fields, without
    /// the external `event_type` wrapper key. Typed round-trip via the
    /// derived `Serialize` impl -- not hand-built `serde_json::json!`.
    pub fn payload_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        let wrapped = serde_json::to_value(self)?;
        Ok(wrapped
            .get(self.event_type())
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    /// Reassembles an `AuditEvent` from a stored `(event_type, payload)`
    /// pair -- the inverse of `event_type()` + `payload_json()`. Used by
    /// the G11 replay surface to reload persisted rows.
    pub fn from_stored(event_type: &str, payload: serde_json::Value) -> Result<Self, serde_json::Error> {
        let wrapped = serde_json::json!({ event_type: payload });
        serde_json::from_value(wrapped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::GateId;

    #[test]
    fn payload_json_round_trips_every_variant() {
        let events = vec![
            AuditEvent::DecisionEvaluated {
                outcome: DecisionOutcome::ApprovedStp,
                snapshot_ref: Some(Uuid::nil()),
                entry_id: Uuid::nil(),
            },
            AuditEvent::EnvelopeSealed {
                envelope_id: Uuid::nil(),
                expires_at: Utc::now(),
            },
            AuditEvent::EnvelopeConsumed {
                envelope_id: Uuid::nil(),
                gate_outcome: GateOutcomeRecord::new(GateId::ExecutionEnvelope, "Success"),
            },
            AuditEvent::DispatchCommitted {
                attested: false,
                gate_outcome: GateOutcomeRecord::new(GateId::WriteSetAttestation, "NotEvaluated"),
            },
            AuditEvent::DispatchRolledBack {
                reason: "test".to_string(),
            },
            AuditEvent::DivergenceTriaged {
                classification: "legacy_defect".to_string(),
                runbook_ref: "R-1".to_string(),
            },
        ];
        for event in events {
            let event_type = event.event_type();
            let payload = event.payload_json().expect("payload serializes");
            let reloaded = AuditEvent::from_stored(event_type, payload).expect("payload deserializes");
            assert_eq!(reloaded, event, "{event_type} did not round-trip");
        }
    }

    /// §3: G10/G14 are the only two gates NOT expected at `ShadowEval`.
    #[test]
    fn expected_provenance_map_is_exhaustive_and_matches_the_doc() {
        for gate in GateId::ALL {
            let provenance = expected_provenance(gate);
            match gate {
                GateId::ExecutionEnvelope => assert_eq!(provenance, GateOutcomeProvenance::ConsumeSeam),
                GateId::WriteSetAttestation => assert_eq!(provenance, GateOutcomeProvenance::PostDispatch),
                _ => assert_eq!(provenance, GateOutcomeProvenance::ShadowEval, "{gate:?}"),
            }
        }
    }

    #[test]
    fn event_provenance_matches_expected_provenance_for_its_own_gate() {
        let consumed = AuditEvent::EnvelopeConsumed {
            envelope_id: Uuid::nil(),
            gate_outcome: GateOutcomeRecord::new(GateId::ExecutionEnvelope, "Success"),
        };
        assert_eq!(consumed.provenance(), Some(GateOutcomeProvenance::ConsumeSeam));
        assert_eq!(expected_provenance(GateId::ExecutionEnvelope), GateOutcomeProvenance::ConsumeSeam);

        let committed = AuditEvent::DispatchCommitted {
            attested: true,
            gate_outcome: GateOutcomeRecord::new(GateId::WriteSetAttestation, "Success"),
        };
        assert_eq!(committed.provenance(), Some(GateOutcomeProvenance::PostDispatch));
        assert_eq!(expected_provenance(GateId::WriteSetAttestation), GateOutcomeProvenance::PostDispatch);
    }
}

//! G13 — Decision Snapshot (V&S §6.15).
//!
//! No production analogue exists today. T3.2 pins every gate read into one
//! object: SemReg snapshot set id, session version/current_snapshot_id,
//! KYC manifest hash + subject next_seq where KYC entities are bound, and
//! entity row_versions where available (RR-5 Mode-1 entities lacking a
//! comparable version pin are capped at `RequiresHumanGate` by the STP
//! classifier, per plan A5).
//!
//! Unlike the other T2/T3 gates, this one has no rejection semantics of its
//! own in §6.15 — its job is to pin whatever was read, not to judge it. The
//! `DecisionSnapshotGate` adapter therefore only distinguishes "pins were
//! supplied" (`Success`) from "no `SnapshotInput` was collected for this
//! evaluation" (`Failure`, fail-closed, same posture as every other gate in
//! this crate) — refusing an empty-but-present pin set is explicitly not
//! this gate's job (a session with no KYC entities bound legitimately has no
//! `kyc_manifest_hash`, for example).

use uuid::Uuid;

use crate::gate::{Gate, GateId, GateResult};

/// T4.4 (G12) — the unified pinned version set: every subsystem version
/// that must match between gate time and execution time, folded into one
/// block on the snapshot rather than left as independently-checked
/// per-subsystem equality tests (C-033's bus catalogue-version check being
/// the one with an existing production analogue; compiler/model/prompt
/// versions have no existing pin at all — see the ledger).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct PinnedVersionSet {
    /// Mirrors `ob-poc-bus-handler`'s `expected_catalogue_version` /
    /// `InvocationContext.catalogue_version` string equality check (C-033).
    pub bus_catalogue_version: Option<String>,
    /// `ob-poc`'s own build version (`CARGO_PKG_VERSION`) at gate time —
    /// the DSL compiler/parser/macro-expander ship as part of the same
    /// crate, so this is the closest existing proxy for "DSL/compiler
    /// crate version" (no finer-grained content hash exists yet).
    pub compiler_version: Option<String>,
    /// From the (net-new, T2.1) interpretation attestation, when present.
    pub model_version: Option<String>,
    pub prompt_version: Option<String>,
}

/// `SnapshotPins` — the artefact pinning every gate read against a
/// specific, reproducible system state, so replaying a persisted
/// `ControlPlaneProof` against the pinned snapshot reproduces the original
/// decision (§12.11). Constructible only from within this module.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SnapshotPins {
    sem_reg_snapshot_id: Option<Uuid>,
    session_snapshot_id: Option<Uuid>,
    kyc_manifest_hash: Option<String>,
    /// Entity id, kind, and observed row_version, for every bound entity
    /// where a version pin is available. An entity absent from this list
    /// has no comparable version pin (RR-5 Mode-1) and is STP-ineligible
    /// per plan A5 until the row-version migration (C-045) lands for its
    /// family.
    ///
    /// T9.2 (EOP-PLAN-CONTROLPLANE-001 Addendum B): widened from
    /// `Vec<(Uuid, i64)>` to include `kind` — neither this pin nor G2's
    /// own `BoundEntities` proof previously carried entity kind anywhere
    /// in the sealed envelope, so a real (non-shadow) pin re-check at
    /// admission time had no honest source for the table to lock, short
    /// of re-deriving it from the verb's *current* args — exactly the
    /// check-time re-derivation the sealed-envelope design exists to
    /// prevent. G13 has zero production callers today, so this widening
    /// is additive with no live caller to break.
    entity_row_versions: Vec<(Uuid, String, i64)>,
    versions: PinnedVersionSet,
}

impl SnapshotPins {
    // Called by the (future) T3.2 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(
        sem_reg_snapshot_id: Option<Uuid>,
        session_snapshot_id: Option<Uuid>,
        kyc_manifest_hash: Option<String>,
        entity_row_versions: Vec<(Uuid, String, i64)>,
        versions: PinnedVersionSet,
    ) -> Self {
        Self {
            sem_reg_snapshot_id,
            session_snapshot_id,
            kyc_manifest_hash,
            entity_row_versions,
            versions,
        }
    }

    pub fn sem_reg_snapshot_id(&self) -> Option<Uuid> {
        self.sem_reg_snapshot_id
    }

    pub fn session_snapshot_id(&self) -> Option<Uuid> {
        self.session_snapshot_id
    }

    pub fn kyc_manifest_hash(&self) -> Option<&str> {
        self.kyc_manifest_hash.as_deref()
    }

    pub fn entity_row_version(&self, entity_id: Uuid) -> Option<i64> {
        self.entity_row_versions
            .iter()
            .find(|(id, _, _)| *id == entity_id)
            .map(|(_, _, version)| *version)
    }

    /// T9.2: entity id + kind + pinned row_version, for every pinned
    /// entity — the single source of truth `verify_pins_in_scope` reads
    /// from directly, rather than requiring a separately-supplied
    /// `entity_kinds` list that could drift from what was actually pinned
    /// at gate time.
    pub fn entity_kinds_and_versions(&self) -> &[(Uuid, String, i64)] {
        &self.entity_row_versions
    }

    pub fn versions(&self) -> &PinnedVersionSet {
        &self.versions
    }

    /// Every bound entity lacking a comparable version pin — the input the
    /// (future) STP classifier (T3.3) uses to enforce plan A5.
    pub fn unpinned_entities<'a>(&'a self, bound: &'a [Uuid]) -> impl Iterator<Item = Uuid> + 'a {
        bound
            .iter()
            .copied()
            .filter(move |id| self.entity_row_version(*id).is_none())
    }
}

#[cfg(any(test, feature = "test-support"))]
pub mod tests_support {
    use super::{PinnedVersionSet, SnapshotPins};
    use uuid::Uuid;

    pub fn pins(
        sem_reg_snapshot_id: Option<Uuid>,
        session_snapshot_id: Option<Uuid>,
        kyc_manifest_hash: Option<String>,
        entity_row_versions: Vec<(Uuid, String, i64)>,
    ) -> SnapshotPins {
        SnapshotPins::new(
            sem_reg_snapshot_id,
            session_snapshot_id,
            kyc_manifest_hash,
            entity_row_versions,
            PinnedVersionSet::default(),
        )
    }

    pub fn pins_with_versions(
        sem_reg_snapshot_id: Option<Uuid>,
        session_snapshot_id: Option<Uuid>,
        kyc_manifest_hash: Option<String>,
        entity_row_versions: Vec<(Uuid, String, i64)>,
        versions: PinnedVersionSet,
    ) -> SnapshotPins {
        SnapshotPins::new(
            sem_reg_snapshot_id,
            session_snapshot_id,
            kyc_manifest_hash,
            entity_row_versions,
            versions,
        )
    }
}

/// Pre-computed input for the decision snapshot gate — the raw pin values
/// the call site collected while resolving the intent (SemReg snapshot set
/// id, session snapshot id, KYC manifest hash, per-entity row versions,
/// and the T4.4 pinned version set).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SnapshotInput {
    pub sem_reg_snapshot_id: Option<Uuid>,
    pub session_snapshot_id: Option<Uuid>,
    pub kyc_manifest_hash: Option<String>,
    pub entity_row_versions: Vec<(Uuid, String, i64)>,
    pub versions: PinnedVersionSet,
}

/// Translates a `SnapshotInput` into `SnapshotPins` — the function T3.4's
/// `ControlPlaneProof` assembly (and any other future consumer that needs
/// the actual pin values, not just a pass/fail signal) calls to obtain the
/// proof.
pub fn build_pins(input: &SnapshotInput) -> SnapshotPins {
    SnapshotPins::new(
        input.sem_reg_snapshot_id,
        input.session_snapshot_id,
        input.kyc_manifest_hash.clone(),
        input.entity_row_versions.clone(),
        input.versions.clone(),
    )
}

/// T3.2 adapter: `Gate<crate::context::EvaluationContext>` impl for G13.
pub struct DecisionSnapshotGate;

impl Gate<crate::context::EvaluationContext> for DecisionSnapshotGate {
    fn id(&self) -> GateId {
        GateId::DecisionSnapshot
    }

    fn evaluate(&self, ctx: &crate::context::EvaluationContext) -> GateResult {
        match &ctx.snapshot {
            Some(_) => GateResult::Success,
            None => GateResult::Failure("no SnapshotInput supplied".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_input_pins_translate_faithfully() {
        let entity = Uuid::nil();
        let input = SnapshotInput {
            sem_reg_snapshot_id: Some(Uuid::nil()),
            session_snapshot_id: None,
            kyc_manifest_hash: Some("hash-1".to_string()),
            entity_row_versions: vec![(entity, "cbu".to_string(), 5)],
            versions: PinnedVersionSet {
                bus_catalogue_version: Some("v3".to_string()),
                ..Default::default()
            },
        };
        let pins = build_pins(&input);
        assert_eq!(pins.sem_reg_snapshot_id(), Some(Uuid::nil()));
        assert_eq!(pins.kyc_manifest_hash(), Some("hash-1"));
        assert_eq!(pins.entity_row_version(entity), Some(5));
        assert_eq!(pins.versions().bus_catalogue_version.as_deref(), Some("v3"));
    }

    #[test]
    fn gate_evaluate_fails_closed_when_input_missing() {
        let ctx = crate::context::EvaluationContext::default();
        assert!(matches!(DecisionSnapshotGate.evaluate(&ctx), GateResult::Failure(_)));
        assert_eq!(DecisionSnapshotGate.id(), GateId::DecisionSnapshot);
    }

    #[test]
    fn gate_evaluate_succeeds_even_with_empty_pins() {
        // A session with no KYC entities bound legitimately has no
        // kyc_manifest_hash — an empty-but-present SnapshotInput must still
        // succeed (this gate pins whatever was read, it doesn't judge it).
        let ctx = crate::context::EvaluationContext {
            snapshot: Some(SnapshotInput::default()),
            ..Default::default()
        };
        assert_eq!(DecisionSnapshotGate.evaluate(&ctx), GateResult::Success);
    }

    #[test]
    fn snapshot_pins_is_constructible_within_its_own_module() {
        let entity = Uuid::nil();
        let pins = SnapshotPins::new(Some(Uuid::nil()), None, None, vec![(entity, "cbu".to_string(), 3)], PinnedVersionSet::default());
        assert_eq!(pins.entity_row_version(entity), Some(3));
    }

    #[test]
    fn unpinned_entities_reports_bound_entities_missing_a_version() {
        let pinned = Uuid::from_u128(1);
        let unpinned = Uuid::from_u128(2);
        let pins = SnapshotPins::new(None, None, None, vec![(pinned, "cbu".to_string(), 1)], PinnedVersionSet::default());
        let result: Vec<Uuid> = pins.unpinned_entities(&[pinned, unpinned]).collect();
        assert_eq!(result, vec![unpinned]);
    }
}

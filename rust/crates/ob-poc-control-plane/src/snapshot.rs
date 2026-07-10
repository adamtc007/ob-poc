//! G13 — Decision Snapshot (V&S §6.15).
//!
//! No production analogue exists today. T3.2 pins every gate read into one
//! object: SemReg snapshot set id, session version/current_snapshot_id,
//! KYC manifest hash + subject next_seq where KYC entities are bound, and
//! entity row_versions where available (RR-5 Mode-1 entities lacking a
//! comparable version pin are capped at `RequiresHumanGate` by the STP
//! classifier, per plan A5).

use uuid::Uuid;

/// `SnapshotPins` — the artefact pinning every gate read against a
/// specific, reproducible system state, so replaying a persisted
/// `ControlPlaneProof` against the pinned snapshot reproduces the original
/// decision (§12.11). Constructible only from within this module.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct SnapshotPins {
    sem_reg_snapshot_id: Option<Uuid>,
    session_snapshot_id: Option<Uuid>,
    kyc_manifest_hash: Option<String>,
    /// Entity id -> observed row_version, for every bound entity where a
    /// version pin is available. An entity absent from this map has no
    /// comparable version pin (RR-5 Mode-1) and is STP-ineligible per plan
    /// A5 until the row-version migration (C-045) lands for its family.
    entity_row_versions: Vec<(Uuid, i64)>,
}

impl SnapshotPins {
    // Called by the (future) T3.2 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(
        sem_reg_snapshot_id: Option<Uuid>,
        session_snapshot_id: Option<Uuid>,
        kyc_manifest_hash: Option<String>,
        entity_row_versions: Vec<(Uuid, i64)>,
    ) -> Self {
        Self {
            sem_reg_snapshot_id,
            session_snapshot_id,
            kyc_manifest_hash,
            entity_row_versions,
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
            .find(|(id, _)| *id == entity_id)
            .map(|(_, version)| *version)
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

#[cfg(test)]
pub(crate) mod tests_support {
    use super::SnapshotPins;
    use uuid::Uuid;

    pub(crate) fn pins(
        sem_reg_snapshot_id: Option<Uuid>,
        session_snapshot_id: Option<Uuid>,
        kyc_manifest_hash: Option<String>,
        entity_row_versions: Vec<(Uuid, i64)>,
    ) -> SnapshotPins {
        SnapshotPins::new(
            sem_reg_snapshot_id,
            session_snapshot_id,
            kyc_manifest_hash,
            entity_row_versions,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_pins_is_constructible_within_its_own_module() {
        let entity = Uuid::nil();
        let pins = SnapshotPins::new(Some(Uuid::nil()), None, None, vec![(entity, 3)]);
        assert_eq!(pins.entity_row_version(entity), Some(3));
    }

    #[test]
    fn unpinned_entities_reports_bound_entities_missing_a_version() {
        let pinned = Uuid::from_u128(1);
        let unpinned = Uuid::from_u128(2);
        let pins = SnapshotPins::new(None, None, None, vec![(pinned, 1)]);
        let result: Vec<Uuid> = pins.unpinned_entities(&[pinned, unpinned]).collect();
        assert_eq!(result, vec![unpinned]);
    }
}

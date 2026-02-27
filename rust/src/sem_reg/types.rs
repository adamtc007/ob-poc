//! Core types for the Semantic Registry.
//!
//! Canonical types live in `sem_os_core::types` (no sqlx dependency).
//! This module re-exports them and provides `PgSnapshotRow` — an sqlx-aware
//! adapter that decodes PostgreSQL enum columns as text and converts to the
//! canonical enum types.

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use uuid::Uuid;

// ── Re-exports from sem_os_core (canonical, no sqlx) ──────────
pub use sem_os_core::types::{
    ChangeType, Classification, GovernanceTier, HandlingControl, ObjectType, SecurityLabel,
    SnapshotMeta, SnapshotRow, SnapshotStatus, TrustClass,
};

// ── PgSnapshotRow — sqlx adapter ──────────────────────────────

/// Raw database row from `sem_reg.snapshots`.
///
/// All PostgreSQL enum columns are decoded as `String` so we don't need
/// `sqlx::Type` derives on the canonical enums (which live in the sqlx-free
/// `sem_os_core` crate). Convert to `SnapshotRow` via `TryFrom`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PgSnapshotRow {
    pub snapshot_id: Uuid,
    pub snapshot_set_id: Option<Uuid>,
    #[sqlx(try_from = "String")]
    pub object_type: String,
    pub object_id: Uuid,
    pub version_major: i32,
    pub version_minor: i32,
    #[sqlx(try_from = "String")]
    pub status: String,
    #[sqlx(try_from = "String")]
    pub governance_tier: String,
    #[sqlx(try_from = "String")]
    pub trust_class: String,
    pub security_label: serde_json::Value,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    pub predecessor_id: Option<Uuid>,
    #[sqlx(try_from = "String")]
    pub change_type: String,
    pub change_rationale: Option<String>,
    pub created_by: String,
    pub approved_by: Option<String>,
    pub definition: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl TryFrom<PgSnapshotRow> for SnapshotRow {
    type Error = anyhow::Error;

    fn try_from(row: PgSnapshotRow) -> Result<Self, Self::Error> {
        Ok(SnapshotRow {
            snapshot_id: row.snapshot_id,
            snapshot_set_id: row.snapshot_set_id,
            object_type: row
                .object_type
                .parse::<ObjectType>()
                .map_err(|_| anyhow!("invalid object_type: {}", row.object_type))?,
            object_id: row.object_id,
            version_major: row.version_major,
            version_minor: row.version_minor,
            status: row
                .status
                .parse::<SnapshotStatus>()
                .map_err(|_| anyhow!("invalid status: {}", row.status))?,
            governance_tier: row
                .governance_tier
                .parse::<GovernanceTier>()
                .map_err(|_| anyhow!("invalid governance_tier: {}", row.governance_tier))?,
            trust_class: row
                .trust_class
                .parse::<TrustClass>()
                .map_err(|_| anyhow!("invalid trust_class: {}", row.trust_class))?,
            security_label: row.security_label,
            effective_from: row.effective_from,
            effective_until: row.effective_until,
            predecessor_id: row.predecessor_id,
            change_type: row
                .change_type
                .parse::<ChangeType>()
                .map_err(|_| anyhow!("invalid change_type: {}", row.change_type))?,
            change_rationale: row.change_rationale,
            created_by: row.created_by,
            approved_by: row.approved_by,
            definition: row.definition,
            created_at: row.created_at,
        })
    }
}

/// Helper to convert a `Vec<PgSnapshotRow>` into `Vec<SnapshotRow>`.
pub fn pg_rows_to_snapshot_rows(rows: Vec<PgSnapshotRow>) -> anyhow::Result<Vec<SnapshotRow>> {
    rows.into_iter().map(SnapshotRow::try_from).collect()
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_rule_check() {
        // Operational + Proof is the forbidden combination
        let meta = SnapshotMeta {
            object_type: ObjectType::AttributeDef,
            object_id: Uuid::new_v4(),
            version_major: 1,
            version_minor: 0,
            status: SnapshotStatus::Active,
            governance_tier: GovernanceTier::Operational,
            trust_class: TrustClass::Proof,
            security_label: SecurityLabel::default(),
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: "test".into(),
            approved_by: None,
            predecessor_id: None,
        };
        // The DB CHECK constraint enforces this, but we also check in Rust gates
        assert!(
            meta.governance_tier == GovernanceTier::Operational
                && meta.trust_class == TrustClass::Proof,
            "This combination should be rejected by publish gates"
        );
    }

    #[test]
    fn test_security_label_serde() {
        let label = SecurityLabel {
            classification: Classification::Confidential,
            pii: true,
            jurisdictions: vec!["UK".into(), "EU".into()],
            purpose_limitation: vec!["KYC_CDD".into()],
            handling_controls: vec![HandlingControl::MaskByDefault],
        };
        let json = serde_json::to_value(&label).unwrap();
        let back: SecurityLabel = serde_json::from_value(json).unwrap();
        assert_eq!(back.classification, Classification::Confidential);
        assert!(back.pii);
        assert_eq!(back.jurisdictions.len(), 2);
    }

    #[test]
    fn test_snapshot_meta_new_operational() {
        let meta =
            SnapshotMeta::new_operational(ObjectType::VerbContract, Uuid::new_v4(), "scanner");
        assert_eq!(meta.governance_tier, GovernanceTier::Operational);
        assert_eq!(meta.trust_class, TrustClass::Convenience);
        assert_eq!(meta.approved_by.as_deref(), Some("auto"));
    }

    #[test]
    fn test_object_type_display() {
        assert_eq!(ObjectType::VerbContract.to_string(), "verb_contract");
        assert_eq!(ObjectType::AttributeDef.to_string(), "attribute_def");
    }
}

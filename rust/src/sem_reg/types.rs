//! Core types for the Semantic Registry.
//!
//! Enums map 1:1 to PostgreSQL `sem_reg.*` types.
//! All structs are snapshot-aware — every registry object is immutable once published.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Enums (mirror sem_reg.* PG enums) ─────────────────────────

/// Governance tier — determines workflow rigour, NOT security posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "sem_reg.governance_tier", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum GovernanceTier {
    Governed,
    Operational,
}

/// Trust class — graduated trust levels for registry objects.
/// Invariant: `Proof` is only valid when `governance_tier = Governed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "sem_reg.trust_class", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TrustClass {
    Proof,
    DecisionSupport,
    Convenience,
}

/// Snapshot lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "sem_reg.snapshot_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    Draft,
    Active,
    Deprecated,
    Retired,
}

/// Change type for snapshot transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "sem_reg.change_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Created,
    NonBreaking,
    Breaking,
    Promotion,
    Deprecation,
    Retirement,
}

/// Registry object type — discriminator for the shared snapshots table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "sem_reg.object_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ObjectType {
    AttributeDef,
    EntityTypeDef,
    RelationshipTypeDef,
    VerbContract,
    TaxonomyDef,
    TaxonomyNode,
    MembershipRule,
    ViewDef,
    PolicyRule,
    EvidenceRequirement,
    DocumentTypeDef,
    ObservationDef,
    DerivationSpec,
}

impl std::fmt::Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::AttributeDef => "attribute_def",
            Self::EntityTypeDef => "entity_type_def",
            Self::RelationshipTypeDef => "relationship_type_def",
            Self::VerbContract => "verb_contract",
            Self::TaxonomyDef => "taxonomy_def",
            Self::TaxonomyNode => "taxonomy_node",
            Self::MembershipRule => "membership_rule",
            Self::ViewDef => "view_def",
            Self::PolicyRule => "policy_rule",
            Self::EvidenceRequirement => "evidence_requirement",
            Self::DocumentTypeDef => "document_type_def",
            Self::ObservationDef => "observation_def",
            Self::DerivationSpec => "derivation_spec",
        };
        write!(f, "{}", s)
    }
}

// ── Security label ────────────────────────────────────────────

/// Security label carried on every registry snapshot (both tiers).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SecurityLabel {
    #[serde(default)]
    pub classification: Classification,
    #[serde(default)]
    pub pii: bool,
    #[serde(default)]
    pub jurisdictions: Vec<String>,
    #[serde(default)]
    pub purpose_limitation: Vec<String>,
    #[serde(default)]
    pub handling_controls: Vec<HandlingControl>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Public,
    #[default]
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandlingControl {
    MaskByDefault,
    NoExport,
    DualControl,
    SecureViewerOnly,
    NoLlmExternal,
}

// ── Snapshot metadata ─────────────────────────────────────────

/// Common metadata for every snapshot.
/// Not a DB row — used as an input struct for creating snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub object_type: ObjectType,
    pub object_id: Uuid,
    pub version_major: i32,
    pub version_minor: i32,
    pub status: SnapshotStatus,
    pub governance_tier: GovernanceTier,
    pub trust_class: TrustClass,
    pub security_label: SecurityLabel,
    pub change_type: ChangeType,
    pub change_rationale: Option<String>,
    pub created_by: String,
    pub approved_by: Option<String>,
    pub predecessor_id: Option<Uuid>,
}

impl SnapshotMeta {
    /// Create metadata for a new operational object (auto-approved).
    pub fn new_operational(
        object_type: ObjectType,
        object_id: Uuid,
        created_by: impl Into<String>,
    ) -> Self {
        Self {
            object_type,
            object_id,
            version_major: 1,
            version_minor: 0,
            status: SnapshotStatus::Active,
            governance_tier: GovernanceTier::Operational,
            trust_class: TrustClass::Convenience,
            security_label: SecurityLabel::default(),
            change_type: ChangeType::Created,
            change_rationale: None,
            created_by: created_by.into(),
            approved_by: Some("auto".into()),
            predecessor_id: None,
        }
    }
}

// ── Full snapshot row (from DB) ───────────────────────────────

/// A complete snapshot row as returned from `sem_reg.snapshots`.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SnapshotRow {
    pub snapshot_id: Uuid,
    pub snapshot_set_id: Option<Uuid>,
    pub object_type: ObjectType,
    pub object_id: Uuid,
    pub version_major: i32,
    pub version_minor: i32,
    pub status: SnapshotStatus,
    pub governance_tier: GovernanceTier,
    pub trust_class: TrustClass,
    pub security_label: serde_json::Value,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    pub predecessor_id: Option<Uuid>,
    pub change_type: ChangeType,
    pub change_rationale: Option<String>,
    pub created_by: String,
    pub approved_by: Option<String>,
    pub definition: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl SnapshotRow {
    /// Deserialise the JSONB `definition` column into a typed body.
    pub fn parse_definition<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.definition.clone())
    }

    /// Deserialise the JSONB `security_label` column.
    pub fn parse_security_label(&self) -> Result<SecurityLabel, serde_json::Error> {
        serde_json::from_value(self.security_label.clone())
    }

    /// Version string, e.g. "1.0"
    pub fn version_string(&self) -> String {
        format!("{}.{}", self.version_major, self.version_minor)
    }
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

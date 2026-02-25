//! SQLx row types for the Semantic OS Postgres adapter.
//!
//! Each row struct derives `sqlx::FromRow` and provides `impl From<Row> for CoreType`.
//! This isolates sqlx dependencies in `sem_os_postgres`, keeping `sem_os_core` pure.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use sem_os_core::types::{
    ChangeType, GovernanceTier, ObjectType, SnapshotRow, SnapshotStatus, TrustClass,
};

// ── Enum string converters ────────────────────────────────────
//
// The core enums have no `sqlx::Type` derive. Postgres stores them as
// custom enum types, so we decode them via `String` columns and convert.

/// Parse a `GovernanceTier` from its Postgres wire string.
pub fn parse_governance_tier(s: &str) -> GovernanceTier {
    match s {
        "governed" => GovernanceTier::Governed,
        "operational" => GovernanceTier::Operational,
        _ => GovernanceTier::Operational, // safe fallback
    }
}

/// Encode a `GovernanceTier` to its Postgres wire string.
pub fn encode_governance_tier(tier: GovernanceTier) -> &'static str {
    match tier {
        GovernanceTier::Governed => "governed",
        GovernanceTier::Operational => "operational",
    }
}

/// Parse a `TrustClass` from its Postgres wire string.
pub fn parse_trust_class(s: &str) -> TrustClass {
    match s {
        "proof" => TrustClass::Proof,
        "decision_support" => TrustClass::DecisionSupport,
        "convenience" => TrustClass::Convenience,
        _ => TrustClass::Convenience, // safe fallback
    }
}

/// Encode a `TrustClass` to its Postgres wire string.
pub fn encode_trust_class(tc: TrustClass) -> &'static str {
    match tc {
        TrustClass::Proof => "proof",
        TrustClass::DecisionSupport => "decision_support",
        TrustClass::Convenience => "convenience",
    }
}

/// Parse a `SnapshotStatus` from its Postgres wire string.
pub fn parse_snapshot_status(s: &str) -> SnapshotStatus {
    match s {
        "draft" => SnapshotStatus::Draft,
        "active" => SnapshotStatus::Active,
        "deprecated" => SnapshotStatus::Deprecated,
        "retired" => SnapshotStatus::Retired,
        _ => SnapshotStatus::Draft, // safe fallback
    }
}

/// Encode a `SnapshotStatus` to its Postgres wire string.
pub fn encode_snapshot_status(status: SnapshotStatus) -> &'static str {
    match status {
        SnapshotStatus::Draft => "draft",
        SnapshotStatus::Active => "active",
        SnapshotStatus::Deprecated => "deprecated",
        SnapshotStatus::Retired => "retired",
    }
}

/// Parse a `ChangeType` from its Postgres wire string.
pub fn parse_change_type(s: &str) -> ChangeType {
    match s {
        "created" => ChangeType::Created,
        "non_breaking" => ChangeType::NonBreaking,
        "breaking" => ChangeType::Breaking,
        "promotion" => ChangeType::Promotion,
        "deprecation" => ChangeType::Deprecation,
        "retirement" => ChangeType::Retirement,
        _ => ChangeType::Created, // safe fallback
    }
}

/// Encode a `ChangeType` to its Postgres wire string.
pub fn encode_change_type(ct: ChangeType) -> &'static str {
    match ct {
        ChangeType::Created => "created",
        ChangeType::NonBreaking => "non_breaking",
        ChangeType::Breaking => "breaking",
        ChangeType::Promotion => "promotion",
        ChangeType::Deprecation => "deprecation",
        ChangeType::Retirement => "retirement",
    }
}

/// Parse an `ObjectType` from its Postgres wire string.
pub fn parse_object_type(s: &str) -> ObjectType {
    match s {
        "attribute_def" => ObjectType::AttributeDef,
        "entity_type_def" => ObjectType::EntityTypeDef,
        "relationship_type_def" => ObjectType::RelationshipTypeDef,
        "verb_contract" => ObjectType::VerbContract,
        "taxonomy_def" => ObjectType::TaxonomyDef,
        "taxonomy_node" => ObjectType::TaxonomyNode,
        "membership_rule" => ObjectType::MembershipRule,
        "view_def" => ObjectType::ViewDef,
        "policy_rule" => ObjectType::PolicyRule,
        "evidence_requirement" => ObjectType::EvidenceRequirement,
        "document_type_def" => ObjectType::DocumentTypeDef,
        "observation_def" => ObjectType::ObservationDef,
        "derivation_spec" => ObjectType::DerivationSpec,
        _ => ObjectType::AttributeDef, // safe fallback
    }
}

/// Encode an `ObjectType` to its Postgres wire string.
pub fn encode_object_type(ot: ObjectType) -> &'static str {
    match ot {
        ObjectType::AttributeDef => "attribute_def",
        ObjectType::EntityTypeDef => "entity_type_def",
        ObjectType::RelationshipTypeDef => "relationship_type_def",
        ObjectType::VerbContract => "verb_contract",
        ObjectType::TaxonomyDef => "taxonomy_def",
        ObjectType::TaxonomyNode => "taxonomy_node",
        ObjectType::MembershipRule => "membership_rule",
        ObjectType::ViewDef => "view_def",
        ObjectType::PolicyRule => "policy_rule",
        ObjectType::EvidenceRequirement => "evidence_requirement",
        ObjectType::DocumentTypeDef => "document_type_def",
        ObjectType::ObservationDef => "observation_def",
        ObjectType::DerivationSpec => "derivation_spec",
    }
}

// ── PgSnapshotRow ─────────────────────────────────────────────

/// Postgres row representation of `sem_reg.snapshots`.
///
/// All enum columns are decoded as `String` (Postgres custom types come
/// over the wire as strings) and converted to core enums in the `From` impl.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PgSnapshotRow {
    pub snapshot_id: Uuid,
    pub snapshot_set_id: Option<Uuid>,
    pub object_type: String,
    pub object_id: Uuid,
    pub version_major: i32,
    pub version_minor: i32,
    pub status: String,
    pub governance_tier: String,
    pub trust_class: String,
    pub security_label: serde_json::Value,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    pub predecessor_id: Option<Uuid>,
    pub change_type: String,
    pub change_rationale: Option<String>,
    pub created_by: String,
    pub approved_by: Option<String>,
    pub definition: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl From<PgSnapshotRow> for SnapshotRow {
    fn from(row: PgSnapshotRow) -> Self {
        SnapshotRow {
            snapshot_id: row.snapshot_id,
            snapshot_set_id: row.snapshot_set_id,
            object_type: parse_object_type(&row.object_type),
            object_id: row.object_id,
            version_major: row.version_major,
            version_minor: row.version_minor,
            status: parse_snapshot_status(&row.status),
            governance_tier: parse_governance_tier(&row.governance_tier),
            trust_class: parse_trust_class(&row.trust_class),
            security_label: row.security_label,
            effective_from: row.effective_from,
            effective_until: row.effective_until,
            predecessor_id: row.predecessor_id,
            change_type: parse_change_type(&row.change_type),
            change_rationale: row.change_rationale,
            created_by: row.created_by,
            approved_by: row.approved_by,
            definition: row.definition,
            created_at: row.created_at,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_governance_tier_round_trip() {
        for tier in [GovernanceTier::Governed, GovernanceTier::Operational] {
            let s = encode_governance_tier(tier);
            assert_eq!(parse_governance_tier(s), tier);
        }
    }

    #[test]
    fn test_trust_class_round_trip() {
        for tc in [
            TrustClass::Proof,
            TrustClass::DecisionSupport,
            TrustClass::Convenience,
        ] {
            let s = encode_trust_class(tc);
            assert_eq!(parse_trust_class(s), tc);
        }
    }

    #[test]
    fn test_snapshot_status_round_trip() {
        for status in [
            SnapshotStatus::Draft,
            SnapshotStatus::Active,
            SnapshotStatus::Deprecated,
            SnapshotStatus::Retired,
        ] {
            let s = encode_snapshot_status(status);
            assert_eq!(parse_snapshot_status(s), status);
        }
    }

    #[test]
    fn test_change_type_round_trip() {
        for ct in [
            ChangeType::Created,
            ChangeType::NonBreaking,
            ChangeType::Breaking,
            ChangeType::Promotion,
            ChangeType::Deprecation,
            ChangeType::Retirement,
        ] {
            let s = encode_change_type(ct);
            assert_eq!(parse_change_type(s), ct);
        }
    }

    #[test]
    fn test_object_type_round_trip() {
        for ot in [
            ObjectType::AttributeDef,
            ObjectType::EntityTypeDef,
            ObjectType::RelationshipTypeDef,
            ObjectType::VerbContract,
            ObjectType::TaxonomyDef,
            ObjectType::TaxonomyNode,
            ObjectType::MembershipRule,
            ObjectType::ViewDef,
            ObjectType::PolicyRule,
            ObjectType::EvidenceRequirement,
            ObjectType::DocumentTypeDef,
            ObjectType::ObservationDef,
            ObjectType::DerivationSpec,
        ] {
            let s = encode_object_type(ot);
            assert_eq!(parse_object_type(s), ot);
        }
    }

    #[test]
    fn test_pg_snapshot_row_conversion() {
        let pg_row = PgSnapshotRow {
            snapshot_id: Uuid::new_v4(),
            snapshot_set_id: None,
            object_type: "verb_contract".to_string(),
            object_id: Uuid::new_v4(),
            version_major: 2,
            version_minor: 1,
            status: "active".to_string(),
            governance_tier: "governed".to_string(),
            trust_class: "proof".to_string(),
            security_label: serde_json::json!({"classification": "confidential", "pii": true}),
            effective_from: Utc::now(),
            effective_until: None,
            predecessor_id: Some(Uuid::new_v4()),
            change_type: "non_breaking".to_string(),
            change_rationale: Some("Updated preconditions".to_string()),
            created_by: "scanner".to_string(),
            approved_by: Some("governance_team".to_string()),
            definition: serde_json::json!({"fqn": "cbu.create"}),
            created_at: Utc::now(),
        };

        let row: SnapshotRow = pg_row.into();
        assert_eq!(row.object_type, ObjectType::VerbContract);
        assert_eq!(row.status, SnapshotStatus::Active);
        assert_eq!(row.governance_tier, GovernanceTier::Governed);
        assert_eq!(row.trust_class, TrustClass::Proof);
        assert_eq!(row.change_type, ChangeType::NonBreaking);
        assert_eq!(row.version_major, 2);
        assert_eq!(row.version_minor, 1);
        assert!(row.predecessor_id.is_some());
        assert_eq!(row.approved_by.as_deref(), Some("governance_team"));
    }

    #[test]
    fn test_fallback_on_unknown_enum_values() {
        assert_eq!(
            parse_governance_tier("unknown"),
            GovernanceTier::Operational
        );
        assert_eq!(parse_trust_class("unknown"), TrustClass::Convenience);
        assert_eq!(parse_snapshot_status("unknown"), SnapshotStatus::Draft);
        assert_eq!(parse_change_type("unknown"), ChangeType::Created);
        assert_eq!(parse_object_type("unknown"), ObjectType::AttributeDef);
    }
}

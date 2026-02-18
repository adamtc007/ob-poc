//! Evidence instance layer — concrete observations, document instances,
//! provenance edges, and retention policies.
//!
//! These types represent the *instance* layer of the evidence framework:
//! while `EvidenceRequirementBody` (Phase 3) defines *what* evidence is needed,
//! this module tracks *actual* evidence artifacts collected for specific entities.
//!
//! ## Tables (migration 090)
//!
//! - `sem_reg.observations` — INSERT-only evidence observations with linear supersession
//! - `sem_reg.document_instances` — concrete document submissions
//! - `sem_reg.provenance_edges` — INSERT-only provenance graph
//! - `sem_reg.retention_policies` — document lifecycle rules

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Observation ───────────────────────────────────────────────

/// Evidence grade — how reliable is this observation?
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGrade {
    PrimaryDocument,
    SecondaryDocument,
    SelfDeclaration,
    ThirdPartyAttestation,
    SystemDerived,
    ManualOverride,
}

impl EvidenceGrade {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PrimaryDocument => "primary_document",
            Self::SecondaryDocument => "secondary_document",
            Self::SelfDeclaration => "self_declaration",
            Self::ThirdPartyAttestation => "third_party_attestation",
            Self::SystemDerived => "system_derived",
            Self::ManualOverride => "manual_override",
        }
    }
}

/// A single evidence observation about an entity, linked to a registry snapshot.
///
/// Observations form a linear supersession chain via `supersedes`.
/// INSERT-only — the DB trigger prevents UPDATE and DELETE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub observation_id: Uuid,
    /// The registry snapshot this observation is evidence for.
    pub snapshot_id: Uuid,
    /// Previous observation in the chain (None for first observation).
    pub supersedes: Option<Uuid>,
    /// When the observation was made.
    pub observed_at: DateTime<Utc>,
    /// Who or what made the observation.
    pub observer_id: String,
    /// Reliability grade of this evidence.
    pub evidence_grade: EvidenceGrade,
    /// Raw evidence payload (document content, API response, etc.)
    pub raw_payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ── Document Instance ─────────────────────────────────────────

/// Status of a document instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentInstanceStatus {
    Pending,
    Received,
    Verified,
    Rejected,
    Expired,
}

impl DocumentInstanceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Received => "received",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
        }
    }
}

/// A concrete document submission linked to a document type definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentInstance {
    pub instance_id: Uuid,
    /// Links to a `document_type_def` snapshot in the registry.
    pub document_type_snapshot_id: Uuid,
    /// The entity this document belongs to.
    pub entity_id: Uuid,
    pub status: DocumentInstanceStatus,
    pub verified_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    /// Reference to external storage (S3 URI, file path, etc.)
    pub storage_ref: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ── Provenance Edge ───────────────────────────────────────────

/// Class of provenance relationship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceEdgeClass {
    DerivedFrom,
    VerifiedBy,
    SupersededBy,
    AttestedBy,
    SourcedFrom,
    ContributedTo,
}

impl ProvenanceEdgeClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DerivedFrom => "derived_from",
            Self::VerifiedBy => "verified_by",
            Self::SupersededBy => "superseded_by",
            Self::AttestedBy => "attested_by",
            Self::SourcedFrom => "sourced_from",
            Self::ContributedTo => "contributed_to",
        }
    }
}

/// A directional provenance edge linking two evidence artifacts.
/// INSERT-only — the DB trigger prevents UPDATE and DELETE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceEdge {
    pub edge_id: Uuid,
    pub source_type: String,
    pub source_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub edge_class: ProvenanceEdgeClass,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ── Retention Policy ──────────────────────────────────────────

/// What to do when the retention period expires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveAction {
    Delete,
    Archive,
    Anonymize,
    Retain,
}

impl ArchiveAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Archive => "archive",
            Self::Anonymize => "anonymize",
            Self::Retain => "retain",
        }
    }
}

/// A retention policy linked to a document type definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub policy_id: Uuid,
    /// Links to a `document_type_def` snapshot in the registry.
    pub document_type_snapshot_id: Uuid,
    pub retention_days: i32,
    pub archive_action: ArchiveAction,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

// ── Store methods ─────────────────────────────────────────────

/// Evidence instance store — DB operations for the evidence instance layer.
pub struct EvidenceInstanceStore;

#[cfg(feature = "database")]
impl EvidenceInstanceStore {
    /// Insert a new observation. Returns the observation_id.
    pub async fn insert_observation(
        pool: &sqlx::PgPool,
        snapshot_id: Uuid,
        observer_id: &str,
        evidence_grade: &EvidenceGrade,
        raw_payload: &serde_json::Value,
        supersedes: Option<Uuid>,
    ) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO sem_reg.observations
               (observation_id, snapshot_id, observer_id, evidence_grade, raw_payload, supersedes)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(id)
        .bind(snapshot_id)
        .bind(observer_id)
        .bind(evidence_grade.as_str())
        .bind(raw_payload)
        .bind(supersedes)
        .execute(pool)
        .await?;
        Ok(id)
    }

    /// Follow the supersession chain for an observation, returning the full chain
    /// from most recent to oldest.
    pub async fn get_observation_chain(
        pool: &sqlx::PgPool,
        observation_id: Uuid,
    ) -> Result<Vec<Observation>, sqlx::Error> {
        // Use recursive CTE to follow the supersession chain
        let rows = sqlx::query_as::<_, ObservationRow>(
            r#"WITH RECURSIVE chain AS (
                SELECT o.* FROM sem_reg.observations o WHERE o.observation_id = $1
                UNION ALL
                SELECT o.* FROM sem_reg.observations o
                JOIN chain c ON o.observation_id = c.supersedes
            )
            SELECT * FROM chain ORDER BY observed_at DESC"#,
        )
        .bind(observation_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Insert a new document instance. Returns the instance_id.
    pub async fn insert_document_instance(
        pool: &sqlx::PgPool,
        document_type_snapshot_id: Uuid,
        entity_id: Uuid,
        status: &DocumentInstanceStatus,
        storage_ref: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO sem_reg.document_instances
               (instance_id, document_type_snapshot_id, entity_id, status, storage_ref, expires_at)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(id)
        .bind(document_type_snapshot_id)
        .bind(entity_id)
        .bind(status.as_str())
        .bind(storage_ref)
        .bind(expires_at)
        .execute(pool)
        .await?;
        Ok(id)
    }

    /// Insert a provenance edge. Returns the edge_id.
    pub async fn insert_provenance_edge(
        pool: &sqlx::PgPool,
        source_type: &str,
        source_id: Uuid,
        target_type: &str,
        target_id: Uuid,
        edge_class: &ProvenanceEdgeClass,
    ) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO sem_reg.provenance_edges
               (edge_id, source_type, source_id, target_type, target_id, edge_class)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
        )
        .bind(id)
        .bind(source_type)
        .bind(source_id)
        .bind(target_type)
        .bind(target_id)
        .bind(edge_class.as_str())
        .execute(pool)
        .await?;
        Ok(id)
    }

    /// Get all provenance edges for a given entity (as source or target).
    pub async fn get_provenance_for_entity(
        pool: &sqlx::PgPool,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<ProvenanceEdge>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ProvenanceEdgeRow>(
            r#"SELECT * FROM sem_reg.provenance_edges
               WHERE (source_type = $1 AND source_id = $2)
                  OR (target_type = $1 AND target_id = $2)
               ORDER BY created_at DESC"#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
}

// ── sqlx row types (feature-gated) ───────────────────────────

#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct ObservationRow {
    observation_id: Uuid,
    snapshot_id: Uuid,
    supersedes: Option<Uuid>,
    observed_at: DateTime<Utc>,
    observer_id: String,
    evidence_grade: String,
    raw_payload: serde_json::Value,
    created_at: DateTime<Utc>,
}

#[cfg(feature = "database")]
impl From<ObservationRow> for Observation {
    fn from(row: ObservationRow) -> Self {
        let grade = match row.evidence_grade.as_str() {
            "primary_document" => EvidenceGrade::PrimaryDocument,
            "secondary_document" => EvidenceGrade::SecondaryDocument,
            "self_declaration" => EvidenceGrade::SelfDeclaration,
            "third_party_attestation" => EvidenceGrade::ThirdPartyAttestation,
            "system_derived" => EvidenceGrade::SystemDerived,
            "manual_override" => EvidenceGrade::ManualOverride,
            _ => EvidenceGrade::ManualOverride, // fallback
        };
        Observation {
            observation_id: row.observation_id,
            snapshot_id: row.snapshot_id,
            supersedes: row.supersedes,
            observed_at: row.observed_at,
            observer_id: row.observer_id,
            evidence_grade: grade,
            raw_payload: row.raw_payload,
            created_at: row.created_at,
        }
    }
}

#[cfg(feature = "database")]
#[derive(sqlx::FromRow)]
struct ProvenanceEdgeRow {
    edge_id: Uuid,
    source_type: String,
    source_id: Uuid,
    target_type: String,
    target_id: Uuid,
    edge_class: String,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
}

#[cfg(feature = "database")]
impl From<ProvenanceEdgeRow> for ProvenanceEdge {
    fn from(row: ProvenanceEdgeRow) -> Self {
        let class = match row.edge_class.as_str() {
            "derived_from" => ProvenanceEdgeClass::DerivedFrom,
            "verified_by" => ProvenanceEdgeClass::VerifiedBy,
            "superseded_by" => ProvenanceEdgeClass::SupersededBy,
            "attested_by" => ProvenanceEdgeClass::AttestedBy,
            "sourced_from" => ProvenanceEdgeClass::SourcedFrom,
            "contributed_to" => ProvenanceEdgeClass::ContributedTo,
            _ => ProvenanceEdgeClass::DerivedFrom, // fallback
        };
        ProvenanceEdge {
            edge_id: row.edge_id,
            source_type: row.source_type,
            source_id: row.source_id,
            target_type: row.target_type,
            target_id: row.target_id,
            edge_class: class,
            metadata: row.metadata,
            created_at: row.created_at,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_grade_roundtrip() {
        let grade = EvidenceGrade::PrimaryDocument;
        assert_eq!(grade.as_str(), "primary_document");

        let json = serde_json::to_value(&grade).unwrap();
        assert_eq!(json, "primary_document");
        let round: EvidenceGrade = serde_json::from_value(json).unwrap();
        assert_eq!(round, EvidenceGrade::PrimaryDocument);
    }

    #[test]
    fn test_document_instance_status_roundtrip() {
        let status = DocumentInstanceStatus::Verified;
        assert_eq!(status.as_str(), "verified");

        let json = serde_json::to_value(&status).unwrap();
        let round: DocumentInstanceStatus = serde_json::from_value(json).unwrap();
        assert_eq!(round, DocumentInstanceStatus::Verified);
    }

    #[test]
    fn test_provenance_edge_class_roundtrip() {
        let class = ProvenanceEdgeClass::DerivedFrom;
        assert_eq!(class.as_str(), "derived_from");

        let json = serde_json::to_value(&class).unwrap();
        let round: ProvenanceEdgeClass = serde_json::from_value(json).unwrap();
        assert_eq!(round, ProvenanceEdgeClass::DerivedFrom);
    }

    #[test]
    fn test_archive_action_roundtrip() {
        let action = ArchiveAction::Anonymize;
        assert_eq!(action.as_str(), "anonymize");

        let json = serde_json::to_value(&action).unwrap();
        let round: ArchiveAction = serde_json::from_value(json).unwrap();
        assert_eq!(round, ArchiveAction::Anonymize);
    }

    #[test]
    fn test_observation_serde() {
        let obs = Observation {
            observation_id: Uuid::new_v4(),
            snapshot_id: Uuid::new_v4(),
            supersedes: None,
            observed_at: Utc::now(),
            observer_id: "agent:kyc-bot".into(),
            evidence_grade: EvidenceGrade::ThirdPartyAttestation,
            raw_payload: serde_json::json!({"source": "gleif", "lei": "529900HNOAA1KXQJUQ27"}),
            created_at: Utc::now(),
        };
        let json = serde_json::to_value(&obs).unwrap();
        let round: Observation = serde_json::from_value(json).unwrap();
        assert_eq!(round.observation_id, obs.observation_id);
        assert_eq!(round.evidence_grade, EvidenceGrade::ThirdPartyAttestation);
        assert!(round.supersedes.is_none());
    }

    #[test]
    fn test_document_instance_serde() {
        let doc = DocumentInstance {
            instance_id: Uuid::new_v4(),
            document_type_snapshot_id: Uuid::new_v4(),
            entity_id: Uuid::new_v4(),
            status: DocumentInstanceStatus::Pending,
            verified_at: None,
            expires_at: None,
            storage_ref: Some("s3://evidence-bucket/passport-001.pdf".into()),
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        };
        let json = serde_json::to_value(&doc).unwrap();
        let round: DocumentInstance = serde_json::from_value(json).unwrap();
        assert_eq!(round.instance_id, doc.instance_id);
        assert_eq!(round.status, DocumentInstanceStatus::Pending);
    }

    #[test]
    fn test_provenance_edge_serde() {
        let edge = ProvenanceEdge {
            edge_id: Uuid::new_v4(),
            source_type: "observation".into(),
            source_id: Uuid::new_v4(),
            target_type: "document_instance".into(),
            target_id: Uuid::new_v4(),
            edge_class: ProvenanceEdgeClass::VerifiedBy,
            metadata: serde_json::json!({"verifier": "compliance-team"}),
            created_at: Utc::now(),
        };
        let json = serde_json::to_value(&edge).unwrap();
        let round: ProvenanceEdge = serde_json::from_value(json).unwrap();
        assert_eq!(round.edge_id, edge.edge_id);
        assert_eq!(round.edge_class, ProvenanceEdgeClass::VerifiedBy);
    }

    #[test]
    fn test_retention_policy_serde() {
        let policy = RetentionPolicy {
            policy_id: Uuid::new_v4(),
            document_type_snapshot_id: Uuid::new_v4(),
            retention_days: 365 * 7, // 7 years
            archive_action: ArchiveAction::Archive,
            metadata: serde_json::json!({"regulation": "GDPR Art. 17"}),
            created_at: Utc::now(),
        };
        let json = serde_json::to_value(&policy).unwrap();
        let round: RetentionPolicy = serde_json::from_value(json).unwrap();
        assert_eq!(round.retention_days, 365 * 7);
        assert_eq!(round.archive_action, ArchiveAction::Archive);
    }
}

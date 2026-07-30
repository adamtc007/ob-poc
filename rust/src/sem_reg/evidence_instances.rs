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
pub(crate) enum EvidenceGrade {
    PrimaryDocument,
    SecondaryDocument,
    SelfDeclaration,
    ThirdPartyAttestation,
    SystemDerived,
    ManualOverride,
}

impl EvidenceGrade {
    pub(crate) fn as_str(&self) -> &'static str {
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

// ── Store methods ─────────────────────────────────────────────

/// Evidence instance store — DB operations for the evidence instance layer.
pub(crate) struct EvidenceInstanceStore;

#[cfg(feature = "database")]
impl EvidenceInstanceStore {
    /// Insert a new observation. Returns the observation_id.
    pub(crate) async fn insert_observation(
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

    // ── Attribute Observation methods ────────────────────────────

    /// Insert a new attribute observation (entity-centric). Returns the observation_id.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn insert_attribute_observation(
        pool: &sqlx::PgPool,
        subject_ref: Uuid,
        attribute_fqn: &str,
        observer_id: &str,
        evidence_grade: &EvidenceGrade,
        confidence: f32,
        snapshot_id: Option<Uuid>,
        raw_payload: Option<&serde_json::Value>,
        supersedes: Option<Uuid>,
    ) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO sem_reg.attribute_observations
               (observation_id, subject_ref, attribute_fqn, snapshot_id,
                confidence, observer_id, evidence_grade, raw_payload, supersedes)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        )
        .bind(id)
        .bind(subject_ref)
        .bind(attribute_fqn)
        .bind(snapshot_id)
        .bind(confidence)
        .bind(observer_id)
        .bind(evidence_grade.as_str())
        .bind(raw_payload)
        .bind(supersedes)
        .execute(pool)
        .await?;
        Ok(id)
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
}

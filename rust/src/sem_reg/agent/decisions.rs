//! Decision records with snapshot provenance chains.
//!
//! A `DecisionRecord` captures a decision made by the agent, including
//! the chosen action, alternatives considered, evidence for/against,
//! and a `snapshot_manifest` that pins every registry snapshot that
//! informed the decision.
//!
//! Decision records are strictly INSERT-only (immutable audit trail).

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ── Decision Record ───────────────────────────────────────────

/// A record of a decision made by the agent.
///
/// The `snapshot_manifest` is the critical provenance chain — a map from
/// object_id to snapshot_id that captures every registry snapshot version
/// that informed this decision. This enables exact replay and audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Unique decision identifier.
    pub decision_id: Uuid,
    /// Plan that this decision was made under (if any).
    #[serde(default)]
    pub plan_id: Option<Uuid>,
    /// Plan step this decision corresponds to (if any).
    #[serde(default)]
    pub step_id: Option<Uuid>,
    /// Reference to the context resolution that informed this decision.
    #[serde(default)]
    pub context_ref: Option<serde_json::Value>,
    /// The action that was chosen.
    pub chosen_action: String,
    /// Description of what was done.
    pub chosen_action_description: String,
    /// Alternative actions that were considered.
    #[serde(default)]
    pub alternatives_considered: Vec<AlternativeAction>,
    /// Evidence supporting this decision.
    #[serde(default)]
    pub evidence_for: Vec<EvidenceItem>,
    /// Evidence against this decision (negative evidence).
    #[serde(default)]
    pub evidence_against: Vec<EvidenceItem>,
    /// Explicitly noted negative evidence (absence of expected data).
    #[serde(default)]
    pub negative_evidence: Vec<EvidenceItem>,
    /// Policy verdicts that were active at decision time (pinned).
    #[serde(default)]
    pub policy_verdicts: Vec<serde_json::Value>,
    /// Complete provenance chain: object_id → snapshot_id.
    ///
    /// Every registry snapshot that was consulted to make this decision
    /// is recorded here. This enables exact-point-in-time audit replay.
    pub snapshot_manifest: HashMap<Uuid, Uuid>,
    /// Confidence in the decision (0.0–1.0).
    pub confidence: f64,
    /// Whether this decision was flagged for human review.
    #[serde(default)]
    pub escalation_flag: bool,
    /// Escalation ID if escalated.
    #[serde(default)]
    pub escalation_id: Option<Uuid>,
    /// Who made this decision (agent ID or user ID).
    pub decided_by: String,
    /// When this decision was made.
    pub decided_at: DateTime<Utc>,
}

/// An alternative action that was considered but not chosen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeAction {
    /// Action identifier (e.g. verb FQN).
    pub action: String,
    /// Why this alternative was not chosen.
    pub reason_rejected: String,
    /// Confidence that this would have been appropriate.
    #[serde(default)]
    pub confidence: Option<f64>,
}

/// A piece of evidence that informed a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// What kind of evidence (observation, document, attribute, etc.).
    pub kind: String,
    /// Reference (FQN or description).
    pub reference: String,
    /// Snapshot ID if the evidence comes from the registry.
    #[serde(default)]
    pub snapshot_id: Option<Uuid>,
    /// Weight of this evidence in the decision (0.0–1.0).
    #[serde(default)]
    pub weight: Option<f64>,
}

// ── Decision Store ────────────────────────────────────────────

/// Database operations for decision records.
pub struct DecisionStore;

impl DecisionStore {
    /// Insert a new decision record (immutable INSERT).
    pub async fn insert(pool: &PgPool, record: &DecisionRecord) -> Result<Uuid> {
        let alternatives_json = serde_json::to_value(&record.alternatives_considered)?;
        let evidence_for_json = serde_json::to_value(&record.evidence_for)?;
        let evidence_against_json = serde_json::to_value(&record.evidence_against)?;
        let negative_evidence_json = serde_json::to_value(&record.negative_evidence)?;
        let policy_verdicts_json = serde_json::to_value(&record.policy_verdicts)?;
        let manifest_json = serde_json::to_value(&record.snapshot_manifest)?;

        let decision_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO sem_reg.decision_records (
                decision_id, plan_id, step_id, context_ref,
                chosen_action, chosen_action_description,
                alternatives_considered, evidence_for, evidence_against,
                negative_evidence, policy_verdicts, snapshot_manifest,
                confidence, escalation_flag, escalation_id,
                decided_by, decided_at
            ) VALUES (
                $1, $2, $3, $4,
                $5, $6,
                $7, $8, $9,
                $10, $11, $12,
                $13, $14, $15,
                $16, $17
            )
            RETURNING decision_id
            "#,
        )
        .bind(record.decision_id)
        .bind(record.plan_id)
        .bind(record.step_id)
        .bind(&record.context_ref)
        .bind(&record.chosen_action)
        .bind(&record.chosen_action_description)
        .bind(&alternatives_json)
        .bind(&evidence_for_json)
        .bind(&evidence_against_json)
        .bind(&negative_evidence_json)
        .bind(&policy_verdicts_json)
        .bind(&manifest_json)
        .bind(record.confidence)
        .bind(record.escalation_flag)
        .bind(record.escalation_id)
        .bind(&record.decided_by)
        .bind(record.decided_at)
        .fetch_one(pool)
        .await?;

        Ok(decision_id)
    }

    /// Load a decision record by ID.
    pub async fn load(pool: &PgPool, decision_id: Uuid) -> Result<Option<DecisionRecord>> {
        let row = sqlx::query_as::<_, DecisionRow>(
            r#"
            SELECT decision_id, plan_id, step_id, context_ref,
                   chosen_action, chosen_action_description,
                   alternatives_considered, evidence_for, evidence_against,
                   negative_evidence, policy_verdicts, snapshot_manifest,
                   confidence, escalation_flag, escalation_id,
                   decided_by, decided_at
            FROM sem_reg.decision_records
            WHERE decision_id = $1
            "#,
        )
        .bind(decision_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some(r) => Ok(Some(r.into_record()?)),
            None => Ok(None),
        }
    }

    /// List decisions for a plan, ordered by time.
    pub async fn list_for_plan(
        pool: &PgPool,
        plan_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DecisionRecord>> {
        let rows = sqlx::query_as::<_, DecisionRow>(
            r#"
            SELECT decision_id, plan_id, step_id, context_ref,
                   chosen_action, chosen_action_description,
                   alternatives_considered, evidence_for, evidence_against,
                   negative_evidence, policy_verdicts, snapshot_manifest,
                   confidence, escalation_flag, escalation_id,
                   decided_by, decided_at
            FROM sem_reg.decision_records
            WHERE plan_id = $1
            ORDER BY decided_at
            LIMIT $2
            "#,
        )
        .bind(plan_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(|r| r.into_record()).collect()
    }

    /// List decisions flagged for escalation.
    pub async fn list_escalated(pool: &PgPool, limit: i64) -> Result<Vec<DecisionRecord>> {
        let rows = sqlx::query_as::<_, DecisionRow>(
            r#"
            SELECT decision_id, plan_id, step_id, context_ref,
                   chosen_action, chosen_action_description,
                   alternatives_considered, evidence_for, evidence_against,
                   negative_evidence, policy_verdicts, snapshot_manifest,
                   confidence, escalation_flag, escalation_id,
                   decided_by, decided_at
            FROM sem_reg.decision_records
            WHERE escalation_flag = true
            ORDER BY decided_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(|r| r.into_record()).collect()
    }
}

// ── Internal DB row type ──────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct DecisionRow {
    decision_id: Uuid,
    plan_id: Option<Uuid>,
    step_id: Option<Uuid>,
    context_ref: Option<serde_json::Value>,
    chosen_action: String,
    chosen_action_description: String,
    alternatives_considered: serde_json::Value,
    evidence_for: serde_json::Value,
    evidence_against: serde_json::Value,
    negative_evidence: serde_json::Value,
    policy_verdicts: serde_json::Value,
    snapshot_manifest: serde_json::Value,
    confidence: f64,
    escalation_flag: bool,
    escalation_id: Option<Uuid>,
    decided_by: String,
    decided_at: DateTime<Utc>,
}

impl DecisionRow {
    fn into_record(self) -> Result<DecisionRecord> {
        Ok(DecisionRecord {
            decision_id: self.decision_id,
            plan_id: self.plan_id,
            step_id: self.step_id,
            context_ref: self.context_ref,
            chosen_action: self.chosen_action,
            chosen_action_description: self.chosen_action_description,
            alternatives_considered: serde_json::from_value(self.alternatives_considered)?,
            evidence_for: serde_json::from_value(self.evidence_for)?,
            evidence_against: serde_json::from_value(self.evidence_against)?,
            negative_evidence: serde_json::from_value(self.negative_evidence)?,
            policy_verdicts: serde_json::from_value(self.policy_verdicts)?,
            snapshot_manifest: serde_json::from_value(self.snapshot_manifest)?,
            confidence: self.confidence,
            escalation_flag: self.escalation_flag,
            escalation_id: self.escalation_id,
            decided_by: self.decided_by,
            decided_at: self.decided_at,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_record_serde_roundtrip() {
        let mut manifest = HashMap::new();
        let obj_id = Uuid::new_v4();
        let snap_id = Uuid::new_v4();
        manifest.insert(obj_id, snap_id);

        let record = DecisionRecord {
            decision_id: Uuid::new_v4(),
            plan_id: Some(Uuid::new_v4()),
            step_id: None,
            context_ref: Some(serde_json::json!({"resolution": "ctx-1"})),
            chosen_action: "ubo.discover".into(),
            chosen_action_description: "Discover UBO structure".into(),
            alternatives_considered: vec![AlternativeAction {
                action: "kyc.open-case".into(),
                reason_rejected: "Case already exists".into(),
                confidence: Some(0.6),
            }],
            evidence_for: vec![EvidenceItem {
                kind: "observation".into(),
                reference: "gleif.import completed".into(),
                snapshot_id: Some(snap_id),
                weight: Some(0.9),
            }],
            evidence_against: vec![],
            negative_evidence: vec![EvidenceItem {
                kind: "missing".into(),
                reference: "No recent PEP screening".into(),
                snapshot_id: None,
                weight: Some(0.5),
            }],
            policy_verdicts: vec![serde_json::json!({
                "policy_fqn": "kyc.pep-check",
                "allowed": true
            })],
            snapshot_manifest: manifest,
            confidence: 0.85,
            escalation_flag: false,
            escalation_id: None,
            decided_by: "agent-1".into(),
            decided_at: Utc::now(),
        };

        let json = serde_json::to_value(&record).unwrap();
        let round: DecisionRecord = serde_json::from_value(json).unwrap();
        assert_eq!(round.decision_id, record.decision_id);
        assert_eq!(round.chosen_action, "ubo.discover");
        assert_eq!(round.snapshot_manifest.len(), 1);
        assert_eq!(round.snapshot_manifest[&obj_id], snap_id);
        assert_eq!(round.alternatives_considered.len(), 1);
        assert_eq!(round.evidence_for.len(), 1);
        assert_eq!(round.negative_evidence.len(), 1);
        assert!((round.confidence - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_alternative_action_serde() {
        let alt = AlternativeAction {
            action: "entity.create".into(),
            reason_rejected: "Entity already exists".into(),
            confidence: Some(0.7),
        };
        let json = serde_json::to_value(&alt).unwrap();
        let round: AlternativeAction = serde_json::from_value(json).unwrap();
        assert_eq!(round.action, "entity.create");
        assert_eq!(round.confidence, Some(0.7));
    }

    #[test]
    fn test_evidence_item_minimal() {
        let item = EvidenceItem {
            kind: "attribute_value".into(),
            reference: "entity.pep-status".into(),
            snapshot_id: None,
            weight: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        let round: EvidenceItem = serde_json::from_value(json).unwrap();
        assert_eq!(round.kind, "attribute_value");
        assert!(round.snapshot_id.is_none());
        assert!(round.weight.is_none());
    }

    #[test]
    fn test_empty_manifest() {
        let record = DecisionRecord {
            decision_id: Uuid::new_v4(),
            plan_id: None,
            step_id: None,
            context_ref: None,
            chosen_action: "test".into(),
            chosen_action_description: "Test action".into(),
            alternatives_considered: vec![],
            evidence_for: vec![],
            evidence_against: vec![],
            negative_evidence: vec![],
            policy_verdicts: vec![],
            snapshot_manifest: HashMap::new(),
            confidence: 1.0,
            escalation_flag: false,
            escalation_id: None,
            decided_by: "test".into(),
            decided_at: Utc::now(),
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["snapshot_manifest"], serde_json::json!({}));
    }
}

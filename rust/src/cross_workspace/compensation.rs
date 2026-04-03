//! Repository for the `compensation_records` table.
//!
//! Regulatory audit trail for every external correction triggered by
//! constellation replay.
//! See: docs/architecture/cross-workspace-state-consistency-v0.4.md §6.5

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

// ── Types ────────────────────────────────────────────────────────────

/// Correction type applied during replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionType {
    Amend,
    CancelRecreate,
    CorrectionFiling,
    Manual,
}

impl CorrectionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Amend => "amend",
            Self::CancelRecreate => "cancel_recreate",
            Self::CorrectionFiling => "correction_filing",
            Self::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "amend" => Ok(Self::Amend),
            "cancel_recreate" => Ok(Self::CancelRecreate),
            "correction_filing" => Ok(Self::CorrectionFiling),
            "manual" => Ok(Self::Manual),
            other => Err(anyhow::anyhow!("Unknown correction type: {other}")),
        }
    }
}

/// Compensation outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationOutcome {
    Success,
    Pending,
    Failed,
}

/// A persisted compensation record row.
#[derive(Debug, Clone, FromRow)]
pub struct CompensationRecordRow {
    pub id: Uuid,
    pub remediation_id: Uuid,
    pub entity_id: Uuid,
    pub provider: String,
    pub original_call_id: Option<Uuid>,
    pub correction_call_id: Option<Uuid>,
    pub correction_type: String,
    pub changed_fields: Option<serde_json::Value>,
    pub outcome: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub confirmed_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Summary for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationSummary {
    pub id: Uuid,
    pub remediation_id: Uuid,
    pub entity_id: Uuid,
    pub provider: String,
    pub correction_type: String,
    pub changed_fields: Option<serde_json::Value>,
    pub outcome: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── Insert ───────────────────────────────────────────────────────────

/// Input for recording a compensation event.
#[derive(Debug, Clone)]
pub struct RecordCompensationInput {
    pub remediation_id: Uuid,
    pub entity_id: Uuid,
    pub provider: String,
    pub original_call_id: Option<Uuid>,
    pub correction_call_id: Option<Uuid>,
    pub correction_type: CorrectionType,
    pub changed_fields: Option<serde_json::Value>,
}

/// Record a compensation event.
pub async fn record_compensation(
    pool: &PgPool,
    input: &RecordCompensationInput,
) -> Result<CompensationRecordRow> {
    let row = sqlx::query_as::<_, CompensationRecordRow>(
        r#"
        INSERT INTO "ob-poc".compensation_records (
            remediation_id, entity_id, provider,
            original_call_id, correction_call_id,
            correction_type, changed_fields
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, remediation_id, entity_id, provider,
                  original_call_id, correction_call_id,
                  correction_type, changed_fields, outcome,
                  confirmed_at, confirmed_by, created_at
        "#,
    )
    .bind(input.remediation_id)
    .bind(input.entity_id)
    .bind(&input.provider)
    .bind(input.original_call_id)
    .bind(input.correction_call_id)
    .bind(input.correction_type.as_str())
    .bind(&input.changed_fields)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

// ── Read ─────────────────────────────────────────────────────────────

/// List compensation records for a remediation event.
pub async fn list_for_remediation(
    pool: &PgPool,
    remediation_id: Uuid,
) -> Result<Vec<CompensationSummary>> {
    let rows = sqlx::query_as::<_, CompensationSummaryRow>(
        r#"
        SELECT id, remediation_id, entity_id, provider,
               correction_type, changed_fields, outcome,
               confirmed_at, created_at
        FROM "ob-poc".compensation_records
        WHERE remediation_id = $1
        ORDER BY created_at
        "#,
    )
    .bind(remediation_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into_summary()).collect())
}

/// Confirm a compensation record (provider acknowledged correction).
pub async fn confirm_compensation(pool: &PgPool, id: Uuid, confirmed_by: &str) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "ob-poc".compensation_records
        SET outcome = 'success',
            confirmed_at = now(),
            confirmed_by = $2
        WHERE id = $1 AND outcome = 'pending'
        "#,
    )
    .bind(id)
    .bind(confirmed_by)
    .execute(pool)
    .await?;
    Ok(())
}

// ── Internal ─────────────────────────────────────────────────────────

#[derive(Debug, FromRow)]
struct CompensationSummaryRow {
    id: Uuid,
    remediation_id: Uuid,
    entity_id: Uuid,
    provider: String,
    correction_type: String,
    changed_fields: Option<serde_json::Value>,
    outcome: String,
    confirmed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl CompensationSummaryRow {
    fn into_summary(self) -> CompensationSummary {
        CompensationSummary {
            id: self.id,
            remediation_id: self.remediation_id,
            entity_id: self.entity_id,
            provider: self.provider,
            correction_type: self.correction_type,
            changed_fields: self.changed_fields,
            outcome: self.outcome,
            confirmed_at: self.confirmed_at,
            created_at: self.created_at,
        }
    }
}
